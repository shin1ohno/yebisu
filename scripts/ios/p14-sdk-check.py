#!/usr/bin/env python3
"""Lease-only native SDK callback tests and a restored final-fence mutant.

Runs ordinary Simulator test processes, NOT actual OS banner delivery. No
permission requests and no app-bundled authorization test is executed here.
"""
import argparse
import hashlib
import json
import os
import signal
from pathlib import Path
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TEST = "platform::notifications::tests::add_check_disconnect_will_present_completion_remove_never_shows"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk-root", type=Path, required=True)
    parser.add_argument("--lease-id", required=True)
    args = parser.parse_args()
    sdk = args.sdk_root.resolve()
    expected = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["dependencies"]["herdr-uikit"]["rev"]
    head = subprocess.check_output(["git", "-C", str(sdk), "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.check_output(["git", "-C", str(sdk), "status", "--porcelain"], text=True).strip()
    if head != expected or dirty:
        raise ValueError("requires clean, exact pinned, exclusively leased SDK checkout")
    out = ROOT / "target/ios-p14-sdk" / time.strftime("%Y%m%d-%H%M%S")
    out.mkdir(parents=True)
    env = os.environ.copy()
    env["CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUNNER"] = str(sdk / "scripts/sim-runner.sh")
    source = sdk / "src/platform/notifications.rs"
    original = source.read_bytes()
    needle = b"ticket.is_some_and(presentation::Ticket::current)"
    if original.count(needle) != 1: raise ValueError("final predicate source no longer matches audited mutant")
    (out / "original.rs").write_bytes(original)
    (out / "run.json").write_text(json.dumps({"lease": args.lease_id, "sdk_head": head, "original_sha256": hashlib.sha256(original).hexdigest(), "actual_os_banner": "UNVERIFIED"}))

    def run(label, test):
        argv = ["cargo", "test", "--locked", "--all-features", "--target", "aarch64-apple-ios-sim", "--lib", test, "--", "--nocapture", "--test-threads=1"]
        result = subprocess.run(argv, cwd=sdk, env=env, text=True, capture_output=True)
        (out / (label + ".json")).write_text(json.dumps({"argv": argv, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}))
        return result

    baseline = run("baseline", TEST)
    if baseline.returncode or "1 passed" not in baseline.stdout:
        raise ValueError("native baseline test did not pass")
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt("native lease runner interrupted; restoring mutant")
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        # Keep the parameter used so -D warnings cannot masquerade as a killed mutant.
        source.write_bytes(original.replace(needle, b"ticket.is_some()"))
        mutant = run("mutant", TEST)
    finally:
        source.write_bytes(original)
        signal.signal(signal.SIGTERM, previous)
    if source.read_bytes() != original: raise ValueError("mutant restoration mismatch")
    restored = run("restored", "platform::notifications::tests::")
    if restored.returncode or "9 passed" not in restored.stdout or "1 ignored" not in restored.stdout:
        raise ValueError("restored native callback suite not fully executed")
    if mutant.returncode != 101 or "FAILED" not in mutant.stdout or "final native display decision" not in mutant.stdout + mutant.stderr:
        raise ValueError("mutant did not fail at the intended no-show assertion")
    (out / "result.json").write_text(json.dumps({"callback_order": "pass", "fence_mutant": "killed at no-show assertion", "restored_sha256": hashlib.sha256(source.read_bytes()).hexdigest(), "actual_os_banner": "UNVERIFIED", "sound": "UNVERIFIED"}, indent=2))


if __name__ == "__main__":
    main()
