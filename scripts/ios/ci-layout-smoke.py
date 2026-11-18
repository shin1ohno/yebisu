#!/usr/bin/env python3
"""The layout-smoke job's driver: the Simulator checks that only a Mac mini ran, on one disposable Simulator.

Built like ci-sh1-152.py: one fresh Simulator, every check run in sequence against it,
the Simulator shut down and deleted whatever happened. Differences, both for the first
measurement of the job's 60-minute budget:
  * a failing check does not stop the others, so one run times and judges all of them;
    the driver still exits non-zero if any check failed;
  * each check's wall time is printed (LAYOUT-SMOKE DURATION) and written to
    target/ios-layout-smoke/durations.json, which the job uploads.
A check in KNOWN_RED still runs and uploads its evidence; its failure is logged as KNOWN RED
and does not fail the job, and its pass does fail the job until the entry is removed.
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
# Integration 1 (2026-11-18) listed p3 here: its landscape stage lost the private-peer
# connection on main aed154e. On the Integration 2 head (run 44214913068, job 134502265733)
# p3 passed every stage, so the entry is gone, as the rule below asks. A check that is red
# for a known reason goes back in here with that reason.
KNOWN_RED = {}
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


def verdict(results):
    """Returns the checks that fail the job: real failures, and known reds that now pass."""
    failed = [r["check"] for r in results if r["exit"] != 0 and r["check"] not in KNOWN_RED]
    fixed = [r["check"] for r in results if r["exit"] == 0 and r["check"] in KNOWN_RED]
    for r in results:
        if r["check"] in KNOWN_RED and r["exit"] != 0:
            print("layout-smoke KNOWN RED:", r["check"], "-", KNOWN_RED[r["check"]], flush=True)
    if fixed:
        print("layout-smoke: a known-red check now passes, remove it from KNOWN_RED:", ", ".join(fixed), flush=True)
    if len(results) != len(CHECKS):
        return failed + fixed + ["a check did not run"]
    return failed + fixed


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
    failed = verdict(results)
    if failed:
        print("layout-smoke FAILED:", ", ".join(failed), flush=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
