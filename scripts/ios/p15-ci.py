#!/usr/bin/env python3
"""Run the P15 local-theme oracle in an isolated GitHub-hosted Simulator."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Use p15-check.py under an explicit exclusive Mac lease outside hosted CI")
    spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
    smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(smoke)
    listing = subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True)
    device_type, runtime = smoke.select_template(json.loads(listing)["devices"])
    device = subprocess.check_output(["xcrun", "simctl", "create", "yebisu P15 CI", device_type, runtime], text=True).strip()
    try:
        subprocess.run([sys.executable, "scripts/ios/p15-check.py"], cwd=ROOT,
                       env={**os.environ, "HERDR_IOS_SIMULATOR": device}, check=True)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)


if __name__ == "__main__":
    main()
