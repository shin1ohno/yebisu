#!/usr/bin/env python3
"""A single disposable Simulator; all menu and regression builds are sequential."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


def main():
    devices = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True))["devices"]
    device_type, runtime = smoke.select_template(devices)
    device = subprocess.check_output(["xcrun", "simctl", "create", "SH1-152 menu acceptance", device_type, runtime], text=True).strip()
    print("SH1-152 simulator", device, runtime, flush=True)
    try:
        for script in ["sh1-152-check.py", "sh1-153-check.py", "p8-native-check.py", "selection-settings-check.py"]:
            subprocess.run([sys.executable, str(Path(__file__).with_name(script))], env={**os.environ, "HERDR_IOS_SIMULATOR": device}, check=True)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print("SH1-152 disposable Simulator deleted", device, flush=True)


if __name__ == "__main__":
    main()
