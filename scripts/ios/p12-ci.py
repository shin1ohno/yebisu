#!/usr/bin/env python3
"""Exercise installed native menus in an isolated GitHub-hosted Simulator."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Use p12-check.py under an explicit exclusive Mac lease outside hosted CI")
    spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
    smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(smoke)
    listing = subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True)
    device_type, runtime = smoke.select_template(json.loads(listing)["devices"])
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
