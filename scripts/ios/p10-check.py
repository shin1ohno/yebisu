#!/usr/bin/env python3
"""P10: native lifecycle callbacks, durable file and process restart receipts."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

_spec = importlib.util.spec_from_file_location("app_stderr", Path(__file__).with_name("app_stderr.py"))
app_stderr = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(app_stderr)
_live_spec = importlib.util.spec_from_file_location("live_jsonl", Path(__file__).with_name("live_jsonl.py"))
live_jsonl = importlib.util.module_from_spec(_live_spec)
_live_spec.loader.exec_module(live_jsonl)


def run(*args, **kwargs):
    print("P10 COMMAND", list(args), flush=True)
    return subprocess.run(args, check=True, text=True, **kwargs)


def validate(paints, saved):
    expected = {"version": 1, "language": "en-US", "theme": "owner-default", "compact": True,
                "terminal_priority": True, "board_hidden": False, "drag_select": True, "copy_on_select": False, "terminal_font_size": None, "terminal_font_family": "SauceCodePro Nerd Font Mono"}
    # settings/store.rs::save always writes Policy::encode, including defaults.
    expected["notifications"] = dict(route="off", sound=True, volume=50, muted=False, quiet=None)
    # SH1-157 L1 (settings/input_options.rs): the QA edit sets the three Input
    # options and Return: Tap; defaults are not written, so these keys appear.
    expected.update(option_key="alt", command_key="ctrl", japanese_candidates="caret", return_policy="tap")
    if saved != expected:
        raise ValueError("settings file differs from source-defined QA edit")
    phases = {p["phase"] for p in paints if p.get("kind") == "settings-lifecycle"}
    if not {"Inactive", "Background", "Active"}.issubset(phases):
        raise ValueError("UIKit lifecycle did not reach the settings adapter")
    loaded = [p for p in paints if p.get("kind") == "settings-loaded"]
    if len(loaded) < 2 or loaded[0]["compact"] or not loaded[-1]["compact"] or not loaded[-1]["terminal_priority"]:
        raise ValueError("fresh process did not restore settings from disk")
    restored = [loaded[-1].get(k) for k in ("option_alt", "command_control", "caret_anchor", "reconnect_automatic")]
    if restored != [True, True, True, False]:
        raise ValueError("fresh process did not restore the Input options and Return from disk")


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
    stderr_start = app_stderr.offset(observations.parent)
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
        # The records and the app's stderr go in the artifact before any oracle runs,
        # so a failure can be read (run 44406830068 job 135164241872 kept neither).
        (directory / "p10-settings.jsonl").write_text("".join(json.dumps(p) + "\n" for p in paints))
        app_stderr.require_no_loss(app_stderr.collect(observations.parent, directory, stderr_start))
        live_jsonl.reject_gaps(paints)
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
