#!/usr/bin/env python3
"""Observe Herdr 2 through UIKit, a private core wire peer and app paint receipts."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def run(*args, **kwargs):
    print("P9 COMMAND", list(args), flush=True)
    return subprocess.run(args, check=True, text=True, **kwargs)


def validate(paints, wire):
    core = [p for p in paints if p.get("kind") == "terminal" and p.get("boot", "").startswith("core-")]
    generations = {p["generation"] for p in core}
    if len(generations) < 2:
        raise ValueError("no core frame after reattachment")
    by_terminal = {}
    for receipt in wire:
        identity = receipt["terminal"]
        by_terminal.setdefault(identity, bytearray()).extend(receipt["bytes"])
    # Source-defined XCTest input and the peer's terminal identities; never
    # derive an expected value from pixels or from a previous successful run.
    first = [identity for identity, data in by_terminal.items() if b"P9-native" in data]
    second = [identity for identity, data in by_terminal.items() if b"P9-pane2" in data]
    if len(first) != 1 or len(second) != 1 or first[0] == second[0]:
        raise ValueError("split input did not reach two distinct terminal IDs")
    if b"P9-pane1" not in by_terminal[first[0]]:
        raise ValueError("reattachment did not preserve the first terminal ID")
    if "かな😀変換".encode() not in by_terminal[first[0]]:
        raise ValueError("IME commit did not reach the first terminal")
    print("P9 receiving-system PASS", "generations", sorted(generations),
          "terminals", {key: list(value) for key, value in by_terminal.items()}, flush=True)


def main():
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    directory = root / "target/ios-p9" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", capture_output=True)
    print(bundle.stderr, flush=True)
    app = bundle.stdout.strip().splitlines()[-1]
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
    run("xcrun", "simctl", "install", device, app)
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    wire_path = container / "Documents/p9-core-wire.jsonl"
    previous = len(wire_path.read_text().splitlines()) if wire_path.exists() else 0
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    try:
        run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
            "-only-testing:SmokeOracle/SmokeQA/testP9Core()", "-destination", "id=" + device,
            "-derivedDataPath", str(root / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
        export = directory / "attachments"
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
        for test in json.loads((export / "manifest.json").read_text()):
            for attachment in test["attachments"]:
                if attachment["suggestedHumanReadableName"].startswith("p9-reattached"):
                    shutil.copyfile(export / attachment["exportedFileName"], directory / "screenshot.png")
        if not (directory / "screenshot.png").is_file():
            raise ValueError("XCTest did not execute the P9 screenshot step")
        paints = [json.loads(line) for line in (container / "Documents/p0-smoke.jsonl").read_text().splitlines()]
        wire = [json.loads(line) for line in wire_path.read_text().splitlines()[previous:]]
        validate(paints, wire)
        (directory / "receipts.json").write_text(json.dumps({"paints": paints, "wire": wire}))
        print("P9 screenshot", directory / "screenshot.png", flush=True)
    finally:
        run("xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu")


if __name__ == "__main__":
    main()
