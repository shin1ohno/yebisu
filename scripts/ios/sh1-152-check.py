#!/usr/bin/env python3
"""Run native menu cases and read actual clipboard, wire and UIKit receipts."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def run(*args, **kwargs):
    print("SH1-152 COMMAND", list(args), flush=True)
    result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    result.check_returncode()
    return result


def main():
    directory = ROOT / "target/ios-sh1-152" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu").stdout.strip().splitlines()[-1]
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').stdout.split()[0]
    run("xcrun", "simctl", "install", device, bundle)
    container = Path(run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").stdout.strip()) / "Documents"
    for path in [*container.glob("sh1-152-*.jsonl"), container / "p4-peer.jsonl", container / "p9-core-wire.jsonl"]:
        path.unlink(missing_ok=True)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    try:
        result = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device,
                     "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(directory / "result.xcresult"),
                     "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/EditMenuQA/testSH1152Basic")
        (directory / "xctest.log").write_text(result.stdout + result.stderr)
        if "testSH1152Basic]' passed" not in result.stdout:
            raise ValueError("native menu XCTest did not execute and pass")
    finally:
        for path in [*container.glob("sh1-152-*.jsonl"), container / "p4-peer.jsonl", container / "p9-core-wire.jsonl"]:
            if path.exists():
                shutil.copyfile(path, directory / path.name)
    read = lambda path: [json.loads(line) for line in path.read_text().splitlines()]
    records = [row for path in directory.glob("sh1-152-*.jsonl") for row in read(path)]
    verdict = oracle.validate_basic(records, read(directory / "p4-peer.jsonl"), read(directory / "p9-core-wire.jsonl"))
    (directory / "verdict.json").write_text(json.dumps({"basic": verdict, "full_acceptance": False}))
    print(verdict, directory, flush=True)


if __name__ == "__main__":
    main()
