#!/usr/bin/env python3
"""Exercise installed native menus in an isolated GitHub-hosted Simulator.

The Simulator is the one the job's P0+P9 step left (ci-smoke.py HANDOVER) when that
step passed: on the xcode-27 image a second fresh Simulator spent 5.4-8.9 min in its
first install (issue #235). The app is uninstalled first, so P12 starts from a new
app container as on a fresh device. Without a handover a new Simulator is created.
Either way this step deletes the Simulator it used.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
BUNDLE_ID = "com.shin1ohno.yebisu"


def load_smoke():
    spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
    smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(smoke)
    return smoke


def available(listing):
    return {entry["udid"] for entries in listing.values() for entry in entries if entry.get("isAvailable")}


def adopt(handover, listing):
    """Returns the handed-over UDID with the app uninstalled, or None (with the reason printed)."""
    if not handover.is_file():
        print("P12 CI no handover from the smoke step; creating a Simulator", flush=True)
        return None
    device = handover.read_text().strip()
    handover.unlink()
    if device not in available(listing):
        print(f"P12 CI handover simulator={device!r} is not an available device; creating a Simulator", flush=True)
        return None
    # P12 reads the app's Documents (p0-smoke.jsonl, p4-peer.jsonl); P0 and P9 must not leave theirs there.
    uninstall = subprocess.run(["xcrun", "simctl", "uninstall", device, BUNDLE_ID], capture_output=True, text=True)
    if uninstall.returncode != 0:
        print(f"P12 CI could not uninstall {BUNDLE_ID} from simulator={device} (exit {uninstall.returncode}: "
              f"{uninstall.stderr.strip()}); deleting it and creating a Simulator", flush=True)
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        return None
    print(f"P12 CI reusing simulator={device} from the smoke step, {BUNDLE_ID} uninstalled", flush=True)
    return device


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Use p12-check.py under an explicit exclusive Mac lease outside hosted CI")
    smoke = load_smoke()
    listing = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True))["devices"]
    device = adopt(smoke.HANDOVER, listing)
    if device is None:
        device_type, runtime = smoke.select_template(listing)
        device = subprocess.check_output(["xcrun", "simctl", "create", "yebisu P12 CI", device_type, runtime], text=True).strip()
    target = ROOT / "target"
    temporary = target / "p12-tmp"
    temporary.mkdir(parents=True, exist_ok=True)
    try:
        subprocess.run([sys.executable, "scripts/ios/p12-check.py", "--output", str(target / "p12-evidence")],
                       env={**os.environ, "CARGO_TARGET_DIR": str(target), "TMPDIR": str(temporary),
                            "HERDR_IOS_SIMULATOR": device}, check=True)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)


if __name__ == "__main__":
    main()
