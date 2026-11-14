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
transition_spec = importlib.util.spec_from_file_location("sh1_152_transitions", Path(__file__).with_name("sh1_152_transitions.py"))
transitions = importlib.util.module_from_spec(transition_spec)
transition_spec.loader.exec_module(transitions)
scene_spec = importlib.util.spec_from_file_location("sh1_152_scenes", Path(__file__).with_name("sh1_152_scenes.py"))
scenes = importlib.util.module_from_spec(scene_spec)
scene_spec.loader.exec_module(scenes)


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
    methods = ["testSH1152Basic", "testSH1152Transitions", "testSH1152DismissAndSystemEdit", "testSH1152TwoScenes"]
    try:
        # A failing XCTest exits non-zero; every receiving-system oracle still runs below.
        print("SH1-152 COMMAND xcodebuild test", flush=True)
        result = subprocess.run(["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device,
                                 "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(directory / "result.xcresult"),
                                 "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/EditMenuQA"], cwd=ROOT, text=True, capture_output=True)
        print(result.stdout, flush=True)
        print(result.stderr, flush=True)
        (directory / "xctest.log").write_text(result.stdout + result.stderr)
    finally:
        for path in [*container.glob("sh1-152-*.jsonl"), container / "p4-peer.jsonl", container / "p9-core-wire.jsonl"]:
            if path.exists():
                shutil.copyfile(path, directory / path.name)
    read = lambda path: [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    records = [row for path in directory.glob("sh1-152-*.jsonl") for row in read(path)]
    peer, core = read(directory / "p4-peer.jsonl"), read(directory / "p9-core-wire.jsonl")
    verdicts, failures = {"xctest": {m: f"{m}]' passed" in result.stdout for m in methods}}, []
    failures += [f"XCTest did not execute and pass: {m}" for m, ok in verdicts["xctest"].items() if not ok]
    checks = {
        "basic": lambda: oracle.validate_basic(records, peer, core),
        "menus": lambda: oracle.visible_menus(result.stdout, [*oracle.BASIC_CASES, *transitions.CASES, "E4-text", "E4-key", "E7", *scenes.CASES]),
        "transitions": lambda: transitions.validate(records, peer, core, result.stdout),
        "scenes": lambda: scenes.validate(records, peer),
    }
    for name, check in checks.items():
        try:
            verdicts[name] = check()
        except (ValueError, KeyError, TypeError, IndexError) as error:
            verdicts[name] = f"FAIL: {type(error).__name__}: {error}"
            failures.append(f"{name}: {error}")
    (directory / "verdict.json").write_text(json.dumps(verdicts, ensure_ascii=False, indent=1))
    print("SH1-152 VERDICT", json.dumps(verdicts, ensure_ascii=False), directory, flush=True)
    if failures:
        raise SystemExit("SH1-152 FAIL\n" + "\n".join(failures))

if __name__ == "__main__":
    main()
