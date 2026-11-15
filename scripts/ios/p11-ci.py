#!/usr/bin/env python3
"""Run the P11 native collector (Text, Drawing, Modal, Disabled) on a hosted macOS Simulator.

The collector never declares full acceptance, so it always exits non-zero with a
fixed closing message. This wrapper treats that exit as a pass only when all four
cases ran, observed controls and left acceptance closed; any other exit fails.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
CASES = ["Text", "Drawing", "Modal", "Disabled"]
CLOSING = "Collected supported controls only; full native acceptance remains CLOSED"


def verdict(returncode, stderr, artifacts):
    """Return None when the collector finished its four cases as designed, else the reason."""
    if returncode != 1 or CLOSING not in stderr:
        return f"collector did not finish its cases (exit {returncode})"
    controls = Path(artifacts) / "source-candidate-controls.json"
    if not controls.is_file():
        return "missing source-candidate-controls.json"
    results = json.loads(controls.read_text())
    if [row.get("case") for row in results] != CASES or not all(row.get("observed_controls") is True for row in results):
        return f"expected observed controls for {CASES}, got {results}"
    acceptance = json.loads((Path(artifacts) / "acceptance.json").read_text())
    if acceptance.get("accepted") is not False:
        return "acceptance must stay closed"
    return None


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Use p11-observer-check.py under an explicit lease outside hosted CI")
    sys.path.insert(0, str(Path(__file__).parent))
    import importlib.util
    spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
    smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(smoke)
    spec = importlib.util.spec_from_file_location("oracle", Path(__file__).with_name("p11-observer-oracle.py"))
    oracle = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(oracle)
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    listing = subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True)
    device_type, runtime = smoke.select_template(json.loads(listing)["devices"])
    device = subprocess.check_output(["xcrun", "simctl", "create", "yebisu P11 CI", device_type, runtime], text=True).strip()
    run_root = ROOT / "target" / "p11-native" / head / str(uuid.uuid4())
    run_root.parent.mkdir(parents=True, exist_ok=True)
    lease = run_root.parent / "lease.json"
    lease.write_text(json.dumps({
        "consumer_head": head, "sdk_head": oracle.SDK, "device": device, "exclusive": True,
        "checkout": str(ROOT.resolve()), "run_root": str(run_root.resolve()),
        "grant": "hosted CI job; the simulator is created for this run only"}, indent=1) + "\n")
    print(f"P11 CI head={head} simulator={device} type={device_type} runtime={runtime}", flush=True)
    try:
        subprocess.run(["xcrun", "simctl", "boot", device], check=True)
        subprocess.run(["xcrun", "simctl", "bootstatus", device, "-b"], check=True)
        env = {k: v for k, v in os.environ.items() if k != "HERDR_IOS_SIMULATOR"}
        result = subprocess.run(
            [sys.executable, str(Path(__file__).with_name("p11-observer-check.py")), "--lease", str(lease),
             "--head", head, "--device", device, "--target-dir", str(run_root)],
            cwd=ROOT, env=env, stderr=subprocess.PIPE, text=True)
        sys.stderr.write(result.stderr)
        reason = verdict(result.returncode, result.stderr, run_root / "artifacts")
        if reason:
            raise SystemExit("P11 native collector FAIL: " + reason)
        print("P11 native collector PASS: Text, Drawing, Modal and Disabled observed controls; acceptance stays closed", flush=True)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=False)


if __name__ == "__main__":
    main()
