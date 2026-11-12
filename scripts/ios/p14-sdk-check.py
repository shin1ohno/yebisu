#!/usr/bin/env python3
"""Lease-only native SDK response/display tests and two independently restored mutants.

Runs ordinary Simulator test processes, NOT actual OS banner delivery. No
permission requests and no app-bundled authorization test is executed here.
"""
import argparse
import hashlib
import json
import os
import re
import signal
from pathlib import Path
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TEST = "platform::notifications::tests::add_check_disconnect_will_present_completion_remove_never_shows"

RESPONSE_TEST = "platform::notifications::response_tests::current_six_second_response_uses_issued_receipt_not_display_age"
RESPONSE_NEEDLE = b"if !responses.owns(tag, now) {"
RESPONSE_MUTANT = b"if !responses.owns(tag, now) || !presentations.get(tag).is_some_and(|ticket| ticket.current()) {"


def require_pass(result, passed, ignored=0):
    summary = rf"test result: ok\. {passed} passed; 0 failed; {ignored} ignored;"
    if result.returncode or not re.search(summary, result.stdout):
        raise ValueError("native suite inventory did not pass")


def require_mutant(result, test, assertion):
    output = result.stdout + result.stderr
    if result.returncode != 101 or f"{test} ... FAILED" not in output or assertion not in output:
        raise ValueError("mutant did not fail at the intended assertion")


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
    consumer_head = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
    if subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain"], text=True).strip():
        raise ValueError("requires clean exact consumer checkout")
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(out / "owned-sdk-target")
    env["CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUNNER"] = str(sdk / "scripts/sim-runner.sh")
    source = sdk / "src/platform/notifications.rs"
    original = source.read_bytes()
    needle = b"ticket.is_some_and(presentation::Ticket::current)"
    if original.count(needle) != 1: raise ValueError("final predicate source no longer matches audited mutant")
    (out / "original.rs").write_bytes(original)
    response_source = sdk / "src/platform/notifications/response.rs"
    response_original = response_source.read_bytes()
    if response_original.count(RESPONSE_NEEDLE) != 1:
        raise ValueError("response source no longer matches audited D5 mutant")
    (out / "response-original.rs").write_bytes(response_original)
    (out / "run.json").write_text(json.dumps({"lease": args.lease_id, "sdk_head": head, "consumer_head": consumer_head, "response_sha256": hashlib.sha256(response_original).hexdigest(), "target_dir": env["CARGO_TARGET_DIR"], "original_sha256": hashlib.sha256(original).hexdigest(), "actual_os_banner": "UNVERIFIED"}))

    def run(label, test):
        argv = ["cargo", "test", "--locked", "--all-features", "--target", "aarch64-apple-ios-sim", "--lib", test, "--", "--nocapture", "--test-threads=1"]
        result = subprocess.run(argv, cwd=sdk, env=env, text=True, capture_output=True)
        (out / (label + ".json")).write_text(json.dumps({"argv": argv, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}))
        return result

    baseline = run("baseline", TEST)
    require_pass(baseline, 1)
    require_pass(run("response-baseline", RESPONSE_TEST), 1)
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
    require_mutant(mutant, TEST, "final native display decision")
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        response_source.write_bytes(response_original.replace(RESPONSE_NEEDLE, RESPONSE_MUTANT))
        response_mutant = run("response-mutant", RESPONSE_TEST)
    finally:
        response_source.write_bytes(response_original)
        signal.signal(signal.SIGTERM, previous)
    if response_source.read_bytes() != response_original:
        raise ValueError("D5 response restoration mismatch")
    require_mutant(response_mutant, RESPONSE_TEST, "D5 six-second SDK response must reach consumer")
    require_pass(run("restored", "platform::notifications::"), 13, 1)
    # Consumer integration runs the same SDK boundary and actual Smoke + private peer.
    consumer_env = env.copy()
    consumer_env["CARGO_TARGET_DIR"] = str(out / "owned-consumer-target")
    argv = ["cargo", "test", "--locked", "--target", "aarch64-apple-ios-sim", "-p", "yebisu",
            "notification_view::response_tests::", "--", "--nocapture", "--test-threads=1"]
    consumer = subprocess.run(argv, cwd=ROOT, env=consumer_env, text=True, capture_output=True)
    (out / "consumer-response.json").write_text(json.dumps({"argv": argv, "exit": consumer.returncode,
        "stdout": consumer.stdout, "stderr": consumer.stderr, "target_dir": consumer_env["CARGO_TARGET_DIR"]}))
    require_pass(consumer, 6)
    (out / "result.json").write_text(json.dumps({"callback_order": "pass", "D5_response_mutant": "killed at six-second callback assertion", "consumer_response_tests": 6, "response_restored_sha256": hashlib.sha256(response_source.read_bytes()).hexdigest(), "fence_mutant": "killed at no-show assertion", "restored_sha256": hashlib.sha256(source.read_bytes()).hexdigest(), "actual_os_banner": "UNVERIFIED", "sound": "UNVERIFIED"}, indent=2))


if __name__ == "__main__":
    main()
