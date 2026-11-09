#!/usr/bin/env python3
"""R3 visible mismatch reason after return, using inbox-only fault injection."""
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[2]
    out = root / "target/ios-r3"
    out.mkdir(exist_ok=True)

    def run(*args):
        print("R3 COMMAND", args, flush=True)
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
                     "-only-testing:SmokeOracle/SmokeQA/testR3Mismatch", "-destination", "id=" + device,
                     "-derivedDataPath", str(Path(work) / "build"), "-resultBundlePath", str(result),
                     "CODE_SIGNING_ALLOWED=NO")
        if "testR3Mismatch]' passed" not in tested:
            raise RuntimeError("R3 XCTest did not execute and pass")
        export = Path(work) / "attachments"
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
        copied = False
        for test in json.loads((export / "manifest.json").read_text()):
            for attachment in test["attachments"]:
                if attachment["suggestedHumanReadableName"].startswith("r3-mismatch-returned"):
                    shutil.copyfile(export / attachment["exportedFileName"], out / "r3-mismatch-returned.png")
                    copied = True
        if not copied:
            raise RuntimeError("missing R3 receiving-system screenshot")
    container = Path(run("xcrun", "simctl", "get_app_container", device,
                         "com.shin1ohno.yebisu", "data").strip())
    records = [json.loads(line) for line in (container / "Documents/p0-smoke.jsonl").read_text().splitlines()]
    phases = [r for r in records if r.get("kind") == "lifecycle"]
    if not any(r["phase"] == "Inactive" for r in phases) or not any(r["phase"] == "Active" for r in phases):
        raise ValueError("R3 did not exercise inactive/active lifecycle")
    if any(r.get("kind") == "reconnect" for r in records):
        raise ValueError("R3 incompatible host was redialed")
    shutil.copyfile(container / "Documents/p0-smoke.jsonl", out / "app.jsonl")
    print("R3 PASS: OCR sees Version mismatch before/after return and after manual tap; no redial")
    print("R3 screenshot", out / "r3-mismatch-returned.png")


if __name__ == "__main__":
    main()
