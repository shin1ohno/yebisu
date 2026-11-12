#!/usr/bin/env python3
"""Lease-only real Appearance taps, painter, close, lifecycle and disk oracle.

Synthetic unit fixtures validate this checker only, never native acceptance.
"""
import importlib.util
import json
from pathlib import Path
import shutil
import time

ROOT = Path(__file__).resolve().parents[2]
SEED = dict(version=1, language="en-US", theme="owner-default", compact=False,
            terminal_priority=False, board_hidden=False, drag_select=True,
            copy_on_select=False, terminal_font_size=None,
            terminal_font_family="SauceCodePro Nerd Font Mono",
            notifications=dict(route="toast", sound=False, volume=37, muted=True, quiet=[60, 120]),
            future=dict(preserve=42))


def validate(records, disk):
    expected = dict(SEED, theme="high-contrast")
    if disk != expected:
        raise ValueError("exact saved schema/theme/other fields mismatch")
    loads = [i for i, r in enumerate(records) if r.get("kind") == "settings-loaded"]
    if len(loads) != 2 or [records[i].get("theme") for i in loads] != ["owner-default", "high-contrast"]:
        raise ValueError("two real process loads must restore the selected theme")
    clicks = [i for i, r in enumerate(records) if r.get("kind") == "theme-selected"]
    if len(clicks) != 1 or records[clicks[0]].get("theme") != "high-contrast":
        raise ValueError("missing unique Appearance click")
    selected = clicks[0]
    lifecycle = [(i, r.get("phase")) for i, r in enumerate(records)
                 if r.get("kind") == "settings-lifecycle" and selected < i < loads[1]]
    background = next((i for i, phase in lifecycle if phase == "Background"), None)
    active = next((i for i, phase in lifecycle if phase == "Active" and background is not None and i > background), None)
    if background is None or active is None:
        raise ValueError("missing background/return lifecycle")
    def painted(start, end, theme, modal):
        return any(r.get("kind") == "terminal" and r.get("theme") == theme
                   and r.get("settings_modal") is modal for r in records[start + 1:end])
    if not loads[0] < selected < background < active < loads[1]:
        raise ValueError("click/lifecycle/relaunch ordering mismatch")
    if not painted(loads[0], selected, "owner-default", False):
        raise ValueError("missing initial owner palette paint")
    if not painted(selected, background, "high-contrast", True):
        raise ValueError("missing immediate modal painter receipt")
    if not painted(selected, background, "high-contrast", False):
        raise ValueError("missing closed-sheet painter receipt")
    if not painted(active, loads[1], "high-contrast", False):
        raise ValueError("missing background return paint")
    if not painted(loads[1], len(records), "high-contrast", False):
        raise ValueError("missing relaunched painter receipt")


def main():
    spec = importlib.util.spec_from_file_location("p11_runner", Path(__file__).with_name("p11-check.py"))
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    run = runner.run
    out = ROOT / "target/ios-p15" / time.strftime("%Y%m%d-%H%M%S")
    out.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu")
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').stdout.split()[0]
    run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").stdout.strip())
    # Only this test namespace is seeded; preserve every pre-existing byte as evidence.
    settings = container / "Library/Application Support/herdr/gpui/p15-qa/yebisu-settings.json"
    settings.parent.mkdir(parents=True, exist_ok=True)
    if settings.exists():
        shutil.copyfile(settings, out / "previous-settings.json")
    settings.write_text(json.dumps(SEED))
    (out / "seed.json").write_text(json.dumps(SEED))
    records_path = container / "Documents/p0-smoke.jsonl"
    previous = len(records_path.read_text().splitlines()) if records_path.exists() else 0
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = out / "result.xcresult"
    try:
        tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"),
                     "-scheme", "SmokeOracle", "-destination", "id=" + device,
                     "-derivedDataPath", str(ROOT / "target/ios-smoke/build"),
                     "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO",
                     "-only-testing:SmokeOracle/SmokeQA/testP15Theme")
    finally:
        # Even failed XCTest/oracle runs retain receiving files before validation.
        records = [json.loads(line) for line in records_path.read_text().splitlines()[previous:]] if records_path.exists() else []
        disk = json.loads(settings.read_text())
        (out / "evidence.json").write_text(json.dumps(dict(records=records, disk=disk), indent=2))
    if "testP15Theme]' passed" not in tested.stdout:
        raise ValueError("P15 XCTest did not execute and pass")
    attachments = out / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(attachments))
    names = [a["suggestedHumanReadableName"] for test in json.loads((attachments / "manifest.json").read_text()) for a in test["attachments"]]
    for stage in ["default", "selected", "closed", "returned", "relaunch"]:
        if not any(name.startswith("p15-" + stage) for name in names):
            raise ValueError("missing native screenshot: " + stage)
    validate(records, disk)
    print("P15 local-theme Simulator oracle passed; host read and physical gates remain unverified.")


if __name__ == "__main__":
    main()
