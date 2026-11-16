#!/usr/bin/env python3
"""Run one edit-menu check (or all, sequentially) on its own disposable Simulator.

Usage: ci-sh1-152.py {sh1-152,sh1-153,p8-native,selection-settings,all}

CI runs each check as its own matrix job, so each gets a fresh runner and a
fresh Simulator. `all` keeps the old sequential run on one Simulator for local use.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)

# Check name -> script. Dict order is the order `all` runs them in.
CHECKS = {
    "sh1-152": "sh1-152-check.py",
    "sh1-153": "sh1-153-check.py",
    "p8-native": "p8-native-check.py",
    "selection-settings": "selection-settings-check.py",
}


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("check", choices=[*CHECKS, "all"], help="check to run, or all of them in sequence")
    return parser.parse_args(argv)


def scripts_for(check):
    return list(CHECKS.values()) if check == "all" else [CHECKS[check]]


def main(argv=None):
    check = parse_args(sys.argv[1:] if argv is None else argv).check
    devices = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True))["devices"]
    device_type, runtime = smoke.select_template(devices)
    device = subprocess.check_output(["xcrun", "simctl", "create", "SH1-152 menu acceptance", device_type, runtime], text=True).strip()
    print("SH1-152 simulator", device, runtime, "check", check, flush=True)
    try:
        for script in scripts_for(check):
            subprocess.run([sys.executable, str(Path(__file__).with_name(script))], env={**os.environ, "HERDR_IOS_SIMULATOR": device}, check=True)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print("SH1-152 disposable Simulator deleted", device, flush=True)


if __name__ == "__main__":
    main()
