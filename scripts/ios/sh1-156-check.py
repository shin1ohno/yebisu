#!/usr/bin/env python3
"""SH1-156: pointer click and Select after a discarded composition, judged by
the private peer's receipts. Simulator UIKit typing, not physical keyboard proof."""
import json
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
TEST = "testSH1156PointerClickAfterDiscard"
# Typed by the XCTest: after the click, then after Select on and off.
EXPECTED = "clicked" + "released"
DISCARDED = "てすと"


def run(*args, **kwargs):
    print("SH1-156 COMMAND", list(args), flush=True)
    result = subprocess.run(args, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    result.check_returncode()
    return result


def typed(messages):
    """Text the terminal received, from commits and plain character keys."""
    text = []
    for message in messages:
        for event in message.get("ClientShellPaneInput", {}).get("events", []):
            if "TextCommit" in event:
                text.append(event["TextCommit"])
            key = event.get("Key")
            if key and key.get("kind") == "Press" and key.get("modifiers") == 0 and "Char" in key.get("code", {}):
                text.append(key["code"]["Char"])
    return "".join(text)


def validate(records, messages):
    discarded = [r for r in records if r.get("kind") == "sh1-156-discarded"]
    if len(discarded) != 1:
        raise ValueError(f"expected one discard fixture record, got {len(discarded)}")
    fixture = discarded[0]
    if fixture.get("marked") != DISCARDED or fixture.get("fenced") is not True:
        raise ValueError(f"the fixture did not leave a discarded composition: {fixture}")
    if fixture.get("drag_select") is not True or fixture.get("select_mode") is not False:
        raise ValueError(f"expected Start: Drag with Select off before the click: {fixture}")
    text = typed(messages)
    if DISCARDED in text:
        raise ValueError(f"the discarded composition reached the terminal: {text!r}")
    if text != EXPECTED:
        raise ValueError(f"letters after the click and Select off: expected {EXPECTED!r}, received {text!r}")
    print("SH1-156 PASS: letters after a pointer click and after Select on/off reached the peer", flush=True)


def main():
    directory = ROOT / "target/ios-sh1-156" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    for name in ["p4-peer.jsonl", "p0-smoke.jsonl"]:
        (container / "Documents" / name).unlink(missing_ok=True)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    try:
        tested = subprocess.run(["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/" + TEST], text=True, capture_output=True)
        print(tested.stdout, flush=True)
        print(tested.stderr, flush=True)
        # Keep the receipts even when the XCTest failed: they show what arrived.
        for source, target in [("p0-smoke.jsonl", "app.jsonl"), ("p4-peer.jsonl", "peer.jsonl")]:
            if (container / "Documents" / source).is_file():
                shutil.copyfile(container / "Documents" / source, directory / target)
        if (directory / "peer.jsonl").is_file():
            received = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
            (directory / "received.txt").write_text(typed(received))
        export = directory / "attachments"
        if result.exists():
            run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
            for test in json.loads((export / "manifest.json").read_text()):
                for attachment in test["attachments"]:
                    stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
                    if stage.startswith("sh1-156-"):
                        shutil.copyfile(export / attachment["exportedFileName"], directory / (stage + ".png"))
        if tested.returncode != 0 or TEST + "]' passed" not in tested.stdout:
            raise RuntimeError("SH1-156 XCTest did not execute and pass")
        records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
        messages = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
        validate(records, messages)
        print("SH1-156 receiving-system evidence", directory, flush=True)
    finally:
        cleanup = subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], text=True, capture_output=True)
        print("SH1-156 cleanup terminate exit", cleanup.returncode, cleanup.stderr, flush=True)


if __name__ == "__main__":
    main()
