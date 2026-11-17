#!/usr/bin/env python3
"""The layout-smoke job's driver: the Simulator checks that only a Mac mini ran, on one disposable Simulator.

Built like ci-sh1-152.py: one fresh Simulator, every check run in sequence against it,
the Simulator shut down and deleted whatever happened. Differences, both for the first
measurement of the job's 60-minute budget:
  * a failing check does not stop the others, so one run times and judges all of them;
    the driver still exits non-zero if any check failed;
  * each check's wall time is printed (LAYOUT-SMOKE DURATION) and written to
    target/ios-layout-smoke/durations.json, which the job uploads.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CHECKS = ["p2-check.py", "p3-check.py", "p10-check.py"]
EVIDENCE = ROOT / "target/ios-layout-smoke"

spec = importlib.util.spec_from_file_location("ci_smoke", HERE / "ci-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


def boot(device):
    """Boots the Simulator and waits for it, as every check expects a booted device it can install into."""
    subprocess.run(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR" >/dev/null'], cwd=ROOT, check=True,
                   env={**os.environ, "HERDR_IOS_SIMULATOR": device})


def run_checks(device):
    """Runs every check, returns [{"check", "seconds", "exit"}] in order."""
    results = []
    for script in CHECKS:
        started = time.monotonic()
        code = subprocess.run([sys.executable, str(HERE / script)], cwd=ROOT, env={**os.environ, "HERDR_IOS_SIMULATOR": device}).returncode
        seconds = round(time.monotonic() - started, 1)
        print("LAYOUT-SMOKE DURATION", script, seconds, "s", "exit", code, flush=True)
        results.append({"check": script, "seconds": seconds, "exit": code})
    return results


def main():
    devices = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True))["devices"]
    device_type, runtime = smoke.select_template(devices)
    device = subprocess.check_output(["xcrun", "simctl", "create", "Layout smoke", device_type, runtime], text=True).strip()
    print("layout-smoke simulator", device, runtime, flush=True)
    started = time.monotonic()
    results = []
    try:
        boot(device)
        results = run_checks(device)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print("layout-smoke disposable Simulator deleted", device, flush=True)
        total = round(time.monotonic() - started, 1)
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        (EVIDENCE / "durations.json").write_text(json.dumps({"runtime": runtime, "total_seconds": total, "checks": results}, indent=2) + "\n")
        print("LAYOUT-SMOKE DURATION total", total, "s", flush=True)
    failed = [r["check"] for r in results if r["exit"] != 0]
    if failed or len(results) != len(CHECKS):
        print("layout-smoke FAILED:", ", ".join(failed) or "a check did not run", flush=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
