#!/usr/bin/env python3
"""SH1-150 Simulator oracle: production selection, pasteboard and screenshot."""
import json
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    print("SH1-150 COMMAND", list(args), flush=True)
    result = subprocess.run(args, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    result.check_returncode()
    return result


def validate(records):
    copies = [r for r in records if r.get("kind") == "sh1-150-copy"]
    if not copies or copies[-1].get("text") != "日本語 ABC":
        raise ValueError("SH1-150 clipboard must equal 日本語 ABC")
    if copies[-1].get("spans") != [[2, {"start": 1, "end": 11}]]:
        raise ValueError("SH1-150 highlight must cover columns 1..11 on row 2")


def main():
    directory = ROOT / "target/ios-cjk" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testSH1150CJK")
    if "testSH1150CJK]' passed" not in tested.stdout:
        raise RuntimeError("SH1-150 XCTest did not execute and pass")
    export = directory / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            if attachment["suggestedHumanReadableName"].startswith("sh1-150-cjk_0_"):
                shutil.copyfile(export / attachment["exportedFileName"], directory / "selection.png")
    shutil.copyfile(container / "Documents/p0-smoke.jsonl", directory / "app.jsonl")
    records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
    validate(records)
    # Verify the oracle rejects the original missing-lead/spurious-space bug.
    try:
        validate([{"kind": "sh1-150-copy", "text": " 本 語  ABC"}])
    except ValueError:
        print("SH1-150 negative oracle PASS", flush=True)
    else:
        raise RuntimeError("negative oracle accepted broken text")
    if not (directory / "selection.png").is_file():
        raise RuntimeError("missing selection screenshot")
    print("SH1-150 receiving-system PASS", json.dumps([r for r in records if r.get("kind") == "sh1-150-copy"], ensure_ascii=False), directory, flush=True)


if __name__ == "__main__":
    main()
