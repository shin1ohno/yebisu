#!/usr/bin/env python3
"""Simulator input adapter + real private-peer receipts, not physical IME proof."""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    print("P4 COMMAND", list(args), flush=True)
    result = subprocess.run(args, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    result.check_returncode()
    return result


def validate(records, messages):
    paints = [r for r in records if r.get("kind") == "terminal"]
    marked = [r for r in paints if r.get("marked") == "かな😀変換"]
    if not marked or any(r["selected"] != [2, 4] for r in marked):
        raise ValueError("P4 marked selection must preserve the emoji UTF-16 clause")
    if not any(not r.get("marked") and r["time"] > marked[-1]["time"] for r in paints):
        raise ValueError("P4 commit must clear rendered composition")
    events = []
    for message in messages:
        if "ClientShellPaneInput" in message:
            body = message["ClientShellPaneInput"]
            if body["pane_id"] != "w1:p1":
                raise ValueError("input reached wrong pane")
            events.extend(body["events"])
    text = "".join(e["TextCommit"] for e in events if "TextCommit" in e)
    if text != "P4-onceかな😀変換":
        raise ValueError("P4 missing, duplicated or uncommitted wire text: " + repr(events))
    print("P4 peer receipts PASS", text, "marked frames", len(marked), flush=True)


def validate_r2(records):
    paints = [r for r in records if r.get("kind") == "terminal"]
    edited = [r for r in paints if r.get("marked") == "かな変換"]
    if not edited or any(r["selected"] != [2, 2] for r in edited):
        raise ValueError("R2 selected Backspace must preserve marked text and UTF-16 caret")
    next_mark = min((r["time"] for r in paints
                     if r.get("marked") == "かな😀変換" and r["time"] > edited[-1]["time"]),
                    default=float("inf"))
    if not any(not r.get("marked") and edited[-1]["time"] < r["time"] < next_mark
               for r in paints):
        raise ValueError("R2 UIKit Esc callback must clear composition")
    print("R2 partial-delete and UIKit cancellation paint receipts PASS", flush=True)


def main():
    directory = ROOT / "target/ios-p4" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    (container / "Documents/p4-peer.jsonl").unlink(missing_ok=True)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testP4Input")
    if "testP4Input]' passed" not in tested.stdout:
        raise RuntimeError("P4 XCTest did not execute and pass")
    export = directory / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("p4-"):
                shutil.copyfile(export / attachment["exportedFileName"], directory / (stage + ".png"))
    for source, target in [("p0-smoke.jsonl", "app.jsonl"), ("p4-peer.jsonl", "peer.jsonl")]:
        shutil.copyfile(container / "Documents" / source, directory / target)
    records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
    validate(records, messages)
    validate_r2(records)
    spec = importlib.util.spec_from_file_location("colors", Path(__file__).with_name("screenshot-color.py"))
    colors = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(colors)
    width, height, channels, rows = colors.decode((directory / "p4-composition.png").read_bytes())
    counts = colors.color_counts(rows, channels)
    # Palette colours from theme.rs, never sampled from the current screenshot.
    for color in ("bf616a", "a3be8c", "ebcb8b", "81a1c1", "b48ead", "88c0d0"):
        if counts.get(bytes.fromhex(color), 0) < 100:
            raise ValueError("missing source palette: " + color)
    print("P4 source-colour oracle PASS", width, height, flush=True)
    print("P4 receiving-system evidence", directory, flush=True)


if __name__ == "__main__":
    main()
