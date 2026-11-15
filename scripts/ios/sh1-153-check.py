#!/usr/bin/env python3
"""Simulator input adapter + real private-peer receipts, not physical IME proof."""
import json
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    print("P8 COMMAND", list(args), flush=True)
    result = subprocess.run(args, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    result.check_returncode()
    return result


def validate(records, messages):
    validations = [r for r in records if r.get("kind") == "edit-validation"]
    results = [r for r in records if r.get("kind") == "edit-result"]
    if [r["case"] for r in validations] != list(range(10)) or [r["case"] for r in results] != list(range(10)):
        raise ValueError("edit cases missing or duplicated")
    segments, current = {}, None
    for message in messages:
        for event in message.get("ClientShellPaneInput", {}).get("events", []):
            text = event.get("Paste", "")
            if text.startswith("edit-start-"):
                case = int(text.removeprefix("edit-start-"))
                if current is not None or case in segments:
                    raise ValueError("overlapping or duplicate start")
                current = case
                segments[current] = []
            elif text.startswith("edit-end-"):
                if current != int(text.removeprefix("edit-end-")):
                    raise ValueError("unmatched end")
                current = None
            elif current is not None:
                segments[current].append(event)
            else:
                raise ValueError("input outside edit action")
    if current is not None or set(segments) != set(range(10)):
        raise ValueError("incomplete edit segments")
    for case, (validation, result) in enumerate(zip(validations, results)):
        probe = validation["probe"]
        enabled = case % 5 in (0, 1, 4)
        if not probe or probe["enabled"] != enabled or probe["validated"] != enabled:
            raise ValueError(f"UIKit enabling mismatch: {case}: {probe}")
        responder = "GPUITextInputView" if case < 5 and case % 5 != 3 else "HerdrUIKitGPUIView"
        if responder not in probe["responder"]:
            raise ValueError(f"wrong keyboard responder: {case}: {probe}")
        clipboard = "日本語" if case % 5 == 1 else f"edit-text-{case}"
        if result["clipboard"] != clipboard:
            raise ValueError(f"clipboard mismatch: {case}")
        expected = [{"Paste": f"edit-text-{case}"}] if case % 5 == 0 else []
        if segments[case] != expected:
            raise ValueError(f"terminal input mismatch: {case}: {segments[case]}")
    print("SH1-153 PASS: UIKit validation, native Paste/Copy, no terminal keys, negative/race cases")


def main():
    directory = ROOT / "target/ios-sh1-153" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    (container / "Documents/p4-peer.jsonl").unlink(missing_ok=True)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testSH1153EditMenu")
    if "testSH1153EditMenu]' passed" not in tested.stdout:
        raise RuntimeError("SH1-153 XCTest did not execute and pass")
    export = directory / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("sh1-153-"):
                shutil.copyfile(export / attachment["exportedFileName"], directory / (stage + ".png"))
    for source, target in [("p0-smoke.jsonl", "app.jsonl"), ("p4-peer.jsonl", "peer.jsonl")]:
        shutil.copyfile(container / "Documents" / source, directory / target)
    records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
    validate(records, messages)
    print("SH1-153 receiving-system evidence", directory, flush=True)


if __name__ == "__main__":
    main()
