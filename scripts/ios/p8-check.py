#!/usr/bin/env python3
"""Simulator input adapter + real private-peer receipts, not physical IME proof."""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import struct
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
    if not any(r.get("kind") == "p8-selection-copy" and r.get("text") == "日本語" for r in records):
        raise ValueError("production selection/copy did not preserve CJK cells")
    if not any(r.get("kind") == "p8-image-source" and r["bytes"] > 16 * 1024 * 1024 for r in records):
        raise ValueError("large image path was not exercised")
    pastes = [e["Paste"] for m in messages if "ClientShellPaneInput" in m
              for e in m["ClientShellPaneInput"]["events"] if "Paste" in e]
    if pastes != ["P8-text-かな", "P8-shadow"]:
        raise ValueError("text paste wire receipt mismatch: " + repr(pastes))
    images = [m["ClipboardImage"] for m in messages if "ClipboardImage" in m]
    if len(images) != 1 or images[0]["extension"] != "png" or len(images[0]["data"]) > 16 * 1024 * 1024:
        raise ValueError("image paste wire receipt missing or oversized")
    payload = bytes(images[0]["data"])
    if images[0]["target"] != {"Pane": "w1:p1"} or payload[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", payload[16:24]) != (1100, 1100):
        raise ValueError("image target or source-defined resized dimensions mismatch")
    if not any(r.get("image_assets") == 1 and r.get("image_placements") == 1 for r in records):
        raise ValueError("SurfaceImages did not reach the painter")
    kinds = []
    following = ""
    for m in messages:
        if "ClipboardImage" in m:
            kinds.append("image")
        if "ClientShellPaneInput" in m:
            for e in m["ClientShellPaneInput"]["events"]:
                if "TextCommit" in e:
                    if "image" not in kinds:
                        raise ValueError("typed input overtook image preparation")
                    following += e["TextCommit"]
    if following != "P8-after":
        raise ValueError("following input lost or duplicated: " + repr(following))
    print("P8 PASS: production selection/copy, text paste, large image received by private peer", flush=True)


def main():
    directory = ROOT / "target/ios-p8" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    (container / "Documents/p4-peer.jsonl").unlink(missing_ok=True)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testP8Handoff")
    if "testP8Handoff]' passed" not in tested.stdout:
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
    spec = importlib.util.spec_from_file_location("colors", Path(__file__).with_name("screenshot-color.py"))
    colors = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(colors)
    width, height, channels, rows = colors.decode((directory / "p8-handoff.png").read_bytes())
    counts = colors.color_counts(rows, channels)
    # Palette colours from theme.rs, never sampled from the current screenshot.
    for color in ("bf616a", "a3be8c", "ebcb8b", "81a1c1", "b48ead", "88c0d0", "ff00aa"):
        if counts.get(bytes.fromhex(color), 0) < 100:
            raise ValueError("missing source palette: " + color)
    print("P8 source-colour oracle PASS", width, height, flush=True)
    print("P8 receiving-system evidence", directory, flush=True)


if __name__ == "__main__":
    main()
