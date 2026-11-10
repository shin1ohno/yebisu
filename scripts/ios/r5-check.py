#!/usr/bin/env python3
"""R5 old reader emits mismatch only after Automatic attempts foreground return."""
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def validate(records):
    phases = [r for r in records if r.get("kind") == "lifecycle"]
    if not any(r["phase"] == "Inactive" for r in phases) or not any(r["phase"] == "Active" for r in phases):
        raise ValueError("R5 did not exercise inactive/active lifecycle")
    releases = [r for r in records if r.get("kind") == "r5-reader-release"]
    if len(releases) != 1 or releases[0].get("pending") is not True or releases[0].get("automatic") is not True:
        raise ValueError("R5 old reader was not held through Automatic return")
    release_index = records.index(releases[0])
    inactive = [r for r in records[:release_index] if r.get("kind") == "lifecycle" and r.get("phase") == "Inactive"]
    active = [r for r in records[release_index+1:] if r.get("kind") == "lifecycle" and r.get("phase") == "Active"]
    if not inactive or not active or not (inactive[-1].get("generation") == releases[0].get("generation") == active[0].get("generation")):
        raise ValueError("R5 reader release did not straddle the same paused generation")
    if any(r.get("kind") == "reconnect" and r.get("host") == "qa@private-peer" for r in records):
        raise ValueError("R5 incompatible host was redialed")


def main():
    root = Path(__file__).resolve().parents[2]
    out = root / "target/ios-r5"
    out.mkdir(exist_ok=True)

    def run(*args):
        print("R5 COMMAND", args, flush=True)
        result = subprocess.run(args, cwd=root, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT)
        print(result.stdout, flush=True)
        result.check_returncode()
        return result.stdout

    built = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu")
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').strip().splitlines()[-1]
    run("xcrun", "simctl", "install", device, built.strip().splitlines()[-1])
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    # Isolated discovery avoids stale XCTest metadata from another phase.
    with tempfile.TemporaryDirectory(prefix="run-", dir=out) as work:
        result = Path(work) / "result.xcresult"
        tested = run("xcodebuild", "test", "-sdk", "iphonesimulator", "-project",
                     str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
                     "-only-testing:SmokeOracle/SmokeQA/testR5LateMismatch", "-destination", "id=" + device,
                     "-derivedDataPath", str(Path(work) / "build"), "-resultBundlePath", str(result),
                     "CODE_SIGNING_ALLOWED=NO")
        if "testR5LateMismatch]' passed" not in tested:
            raise RuntimeError("R5 XCTest did not execute and pass")
        export = Path(work) / "attachments"
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
        copied = False
        for test in json.loads((export / "manifest.json").read_text()):
            for attachment in test["attachments"]:
                if attachment["suggestedHumanReadableName"].startswith("r5-late-mismatch-returned"):
                    shutil.copyfile(export / attachment["exportedFileName"], out / "r5-late-mismatch-returned.png")
                    copied = True
        if not copied:
            raise RuntimeError("missing R5 receiving-system screenshot")
    container = Path(run("xcrun", "simctl", "get_app_container", device,
                         "com.shin1ohno.yebisu", "data").strip())
    records = [json.loads(line) for line in (container / "Documents/p0-smoke.jsonl").read_text().splitlines()]
    validate(records)
    shutil.copyfile(container / "Documents/p0-smoke.jsonl", out / "app.jsonl")
    print("R5 PASS: OCR sees delayed Version mismatch after Automatic return and manual tap; no selected-host redial; old reader released after Automatic guard")
    print("R5 screenshot", out / "r5-late-mismatch-returned.png")


if __name__ == "__main__":
    main()
