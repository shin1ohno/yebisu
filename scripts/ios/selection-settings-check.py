#!/usr/bin/env python3
"""SH1-151: real strip taps, rendered labels, disk and fresh-process restore."""
import json
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    print("SH1-151 COMMAND", list(args), flush=True)
    result = subprocess.run(args, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    result.check_returncode()
    return result


def validate(records, saved):
    loaded = [r for r in records if r.get("kind") == "settings-loaded"]
    if len(loaded) != 2:
        raise ValueError("expected two process loads")
    if (loaded[0].get("drag_select"), loaded[0].get("copy_on_select")) != (True, False):
        raise ValueError("fresh defaults must be Drag and Manual")
    if (loaded[1].get("drag_select"), loaded[1].get("copy_on_select")) != (False, True):
        raise ValueError("relaunch must restore both strip edits")
    expected = dict(version=1, language="en-US", theme="owner-default", compact=False, terminal_priority=False,
                    board_hidden=False, drag_select=False, copy_on_select=True, terminal_font_size=None, terminal_font_family="SauceCodePro Nerd Font Mono")
    if saved != expected:
        raise ValueError("receiving file must contain both strip edits and unchanged settings")


def main():
    directory = ROOT / "target/ios-selection-settings" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", cwd=ROOT)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], cwd=ROOT, text=True).split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    settings = container / "Library/Application Support/herdr/gpui/sh1-151-qa"
    # Reset only this oracle's isolated namespace; preserve ordinary app state.
    if settings.exists():
        shutil.rmtree(settings)
    observations = container / "Documents/selection-settings.jsonl"
    previous = len(observations.read_text().splitlines()) if observations.exists() else 0
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    try:
        tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testSH1151SelectionSettings()")
        if "testSH1151SelectionSettings]' passed" not in tested.stdout:
            raise RuntimeError("SH1-151 XCTest did not execute and pass")
        export = directory / "attachments"
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
        for test in json.loads((export / "manifest.json").read_text()):
            for attachment in test["attachments"]:
                for stage in ["fresh", "restored"]:
                    if attachment["suggestedHumanReadableName"].startswith("sh1-151-" + stage + "_0_"):
                        shutil.copyfile(export / attachment["exportedFileName"], directory / (stage + ".png"))
        records = [json.loads(line) for line in observations.read_text().splitlines()[previous:]]
        saved = json.loads((settings / "yebisu-settings.json").read_text())
        validate(records, saved)
        (directory / "receipts.json").write_text(json.dumps(dict(records=records, saved=saved)))
        for stage in ["fresh", "restored"]:
            if not (directory / (stage + ".png")).is_file():
                raise RuntimeError("missing screenshot: " + stage)
        print("SH1-151 receiving-system PASS", json.dumps(saved), directory, flush=True)
    finally:
        # An unlaunched/already exited app has nothing to terminate (code 3).
        cleanup = subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], text=True, capture_output=True)
        print("SH1-151 cleanup terminate exit", cleanup.returncode, cleanup.stderr, flush=True)
        if settings.exists():
            shutil.rmtree(settings)
        if cleanup.returncode not in (0, 3):
            cleanup.check_returncode()


if __name__ == "__main__":
    main()
