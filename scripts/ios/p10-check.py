#!/usr/bin/env python3
"""P10: native lifecycle callbacks, durable file and process restart receipts."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def run(*args, **kwargs):
    print("P10 COMMAND", list(args), flush=True)
    return subprocess.run(args, check=True, text=True, **kwargs)


def validate(paints, saved):
    expected = {"version": 1, "language": "en-US", "compact": True,
                "terminal_priority": True, "board_hidden": False, "drag_select": True, "copy_on_select": False}
    if saved != expected:
        raise ValueError("settings file differs from source-defined QA edit")
    phases = {p["phase"] for p in paints if p.get("kind") == "settings-lifecycle"}
    if not {"Inactive", "Background", "Active"}.issubset(phases):
        raise ValueError("UIKit lifecycle did not reach the settings adapter")
    loaded = [p for p in paints if p.get("kind") == "settings-loaded"]
    if len(loaded) < 2 or loaded[0]["compact"] or not loaded[-1]["compact"] or not loaded[-1]["terminal_priority"]:
        raise ValueError("fresh process did not restore settings from disk")


def main():
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    directory = root / "target/ios-p10" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", capture_output=True)
    print(bundle.stderr, flush=True)
    app = bundle.stdout.strip().splitlines()[-1]
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
    run("xcrun", "simctl", "install", device, app)
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    settings = container / "Library/Application Support/herdr/gpui/p10-qa"
    # Only this test's isolated settings are reset; ordinary app state is retained.
    if settings.exists():
        shutil.rmtree(settings)
    observations = container / "Documents/p10-settings.jsonl"
    previous = len(observations.read_text().splitlines()) if observations.exists() else 0
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = directory / "result.xcresult"
    try:
        run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
            "-only-testing:SmokeOracle/SmokeQA/testP10Settings()", "-destination", "id=" + device,
            "-derivedDataPath", str(root / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
        export = directory / "attachments"
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
        for test in json.loads((export / "manifest.json").read_text()):
            for attachment in test["attachments"]:
                if attachment["suggestedHumanReadableName"].startswith("p10-restored"):
                    shutil.copyfile(export / attachment["exportedFileName"], directory / "screenshot.png")
        if not (directory / "screenshot.png").is_file():
            raise ValueError("missing P10 screenshot attachment")
        paints = [json.loads(line) for line in observations.read_text().splitlines()[previous:]]
        saved = json.loads((settings / "yebisu-settings.json").read_text())
        validate(paints, saved)
        (directory / "receipts.json").write_text(json.dumps({"paints": paints, "saved": saved}))
        print("P10 receiving-system PASS", json.dumps(saved), flush=True)
        print("P10 screenshot", directory / "screenshot.png", flush=True)
    finally:
        run("xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu")
        if settings.exists():
            shutil.rmtree(settings)


if __name__ == "__main__":
    main()
