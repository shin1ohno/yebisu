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
    starts = [r for r in records if r.get("kind") == "native-start"]
    results = [r for r in records if r.get("kind") == "native-result"]
    if [r["case"] for r in starts] != list(range(24)) or [r["case"] for r in results] != list(range(24)):
        raise ValueError("native selector cases missing or duplicated")
    segments = {}
    current = None
    for m in messages:
        if "ClientShellPaneInput" not in m:
            continue
        for event in m["ClientShellPaneInput"]["events"]:
            text = event.get("Paste", "")
            if text.startswith("native-start-"):
                current = int(text.removeprefix("native-start-"))
                if current in segments:
                    raise ValueError("duplicate wire start")
                segments[current] = []
            elif text.startswith("native-end-"):
                if current != int(text.removeprefix("native-end-")):
                    raise ValueError("wire end mismatch")
                current = None
            elif current is not None:
                segments[current].append(event)
            else:
                raise ValueError("unexpected input outside native action")
    for i, (start, result) in enumerate(zip(starts, results)):
        if not start["scheduled"] or start["command_control"] != (i//6 >= 2) or start["terminal_priority"] != (i//6%2 == 1):
            raise ValueError("selector or setting mismatch")
        if start["action"] != ["paste:", "copy:", "cut:", "selectAll:", "physical-v", "physical-c"][i%6]:
            raise ValueError("selector mismatch")
        terminal_route = i//6 == 3
        expected = "日本語" if i%6 in (1,2) or (i%6 == 5 and not terminal_route) else f"native-text-{i}"
        if result["preparing"] or result["clipboard"] != expected:
            raise ValueError(f"native clipboard mismatch case {i}: {result}")
        actual = segments.get(i)
        if i%6 in (4,5) and terminal_route:
            key = "v" if i%6 == 4 else "c"
            if actual is None or len(actual) != 2 or any(
                event.get("Key", {}).get("code") != {"Char":key}
                or event["Key"]["modifiers"] != 2 or event["Key"]["kind"] != kind
                for event, kind in zip(actual, ["Press", "Release"])):
                raise ValueError(f"physical Ctrl key mismatch case {i}: {actual}")
            continue
        expected_events = [{"Paste": f"native-text-{i}"}] if i%6 in (0,4) else []
        if actual != expected_events:
            raise ValueError(f"native wire mismatch case {i}: {segments.get(i)}")
    if current is not None:
        raise ValueError("wire segment unfinished")
    print("NATIVE PASS: 4 settings x native Paste/Copy/Cut/SelectAll and physical C/V adapters", flush=True)


def main():
    directory = ROOT / "target/ios-p8-native" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    (container / "Documents/p4-peer.jsonl").unlink(missing_ok=True)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testP8NativeEditing")
    if "testP8NativeEditing]' passed" not in tested.stdout:
        raise RuntimeError("P8 XCTest did not execute and pass")
    export = directory / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("p8-"):
                shutil.copyfile(export / attachment["exportedFileName"], directory / (stage + ".png"))
    for source, target in [("p0-smoke.jsonl", "app.jsonl"), ("p4-peer.jsonl", "peer.jsonl")]:
        shutil.copyfile(container / "Documents" / source, directory / target)
    records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
    validate(records, messages)
    print("NATIVE receiving-system evidence", directory, flush=True)


if __name__ == "__main__":
    main()
