#!/usr/bin/env python3
"""A single disposable Simulator; all menu and regression builds are sequential."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)
crash_spec = importlib.util.spec_from_file_location("crash_reports", Path(__file__).with_name("crash_reports.py"))
crash_reports = importlib.util.module_from_spec(crash_spec)
crash_spec.loader.exec_module(crash_reports)
ready_spec = importlib.util.spec_from_file_location("sim_ready", Path(__file__).with_name("sim_ready.py"))
sim_ready = importlib.util.module_from_spec(ready_spec)
ready_spec.loader.exec_module(sim_ready)
EVIDENCE = Path(__file__).resolve().parents[2] / "target" / "ios-sh1-152" / "crash-reports"


def main():
    started = time.time()
    devices = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True))["devices"]
    device_type, runtime = smoke.select_template(devices)
    device = subprocess.check_output(["xcrun", "simctl", "create", "SH1-152 menu acceptance", device_type, runtime], text=True).strip()
    print("SH1-152 simulator", device, runtime, flush=True)
    # Issue #256: lib.sh keeps the boot-task daemons out of this disposable device when it boots it,
    # and the watch reports whether they run anyway.
    stop_watch = sim_ready.start_boot_task_watch(device)
    env = {**os.environ, "HERDR_IOS_SIMULATOR": device, "HERDR_IOS_QUIET_BOOT_TASKS": "1"}
    try:
        for script in ["sh1-152-check.py", "sh1_152_targets.py", "sh1-153-check.py", "p8-native-check.py", "selection-settings-check.py", "sh1-156-check.py"]:
            subprocess.run([sys.executable, str(Path(__file__).with_name(script))], env=env, check=True)
    finally:
        stop_watch.set()
        # Issue #261: a SpringBoard or app crash leaves its report only on this VM.
        copied = crash_reports.copy_since(started, EVIDENCE)
        print("SH1-152 crash reports", len(copied), *copied, flush=True)
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print("SH1-152 disposable Simulator deleted", device, flush=True)


if __name__ == "__main__":
    main()
