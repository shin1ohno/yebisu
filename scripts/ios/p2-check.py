#!/usr/bin/env python3
"""Run P2 UI on Simulator with an injected key; never read keys from logs.

UI test redesign S2 (owner decision ui-test-redesign-decision.json). testP2KeysAndRestart launches
the app itself, twice with the same environment, finds the key block's controls by QA identifier
and confirms every press through qa.state. This driver only installs the app, runs XCTest,
terminates the app and then reads p2-setup.jsonl once, whole: it never reads a file the app may
still be writing (runs 44155672313, 44236636053, 44288317467 failed on such reads). The pasteboard
is checked by what the app read back from its own UIPasteboard right after Copy, as a SHA-256 and
a length, on a hosted runner and on the Mac mini alike: XCTest compares qa.state, and this driver
compares the p2-pasteboard receipt. simctl pbpaste, which times out on hosted runners (run
44151000668 job 134279069207), is only a Mac mini aid now.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import importlib.util

_ready_spec = importlib.util.spec_from_file_location("sim_ready", Path(__file__).with_name("sim_ready.py"))
sim_ready = importlib.util.module_from_spec(_ready_spec)
_ready_spec.loader.exec_module(sim_ready)
_budget_spec = importlib.util.spec_from_file_location("resend_budget", Path(__file__).with_name("resend_budget.py"))
resend_budget = importlib.util.module_from_spec(_budget_spec)
_budget_spec.loader.exec_module(resend_budget)

BUNDLE = "com.shin1ohno.yebisu"
# The line the injected QA provider holds (crates/yebisu/src/settings/key_ops/provider.rs).
KEY_LINE = "restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa"
COPIED = "Copied restricted public key."


def run(*command, **kwargs):
    print("P2 COMMAND", list(command), flush=True)
    if command[:2] == ("xcodebuild", "test"):
        # Issue #142 stage 4: streamed as before, and kept for the job's resend budget.
        # Class D: once more when xcodebuild found no device and ran no test (sim_ready.run_test).
        result = sim_ready.run_test(list(command), lambda: subprocess.CompletedProcess(list(command), *resend_budget.run_tee(list(command), **kwargs)))
        resend_budget.record(Path(__file__).stem, result.stdout)
        if result.returncode:
            raise subprocess.CalledProcessError(result.returncode, command, result.stdout)
        return result
    return subprocess.run(command, check=True, text=True, **kwargs)


def expected_pasteboard(line=KEY_LINE):
    """(sha256 hex, byte length) of the key line as UTF-8: what the app must read back after Copy."""
    data = line.encode()
    return hashlib.sha256(data).hexdigest(), len(data)


def validate_preflight(records):
    if not any(r.get("kind")=="key-provider-preflight" and r.get("phase")=="startup" and r.get("injected") is True for r in records):
        raise RuntimeError("runtime injected provider preflight missing before key operation")

def validate_records(records):
    groups = []
    for record in records:
        if record["kind"] == "launch":
            groups.append([])
        elif groups:
            groups[-1].append(record)
    if len(groups) < 2:
        raise RuntimeError("missing independent process launch receipts")
    if any("key_line" in record for record in records):
        raise RuntimeError("public key leaked into observation")
    for group in groups:
        validate_preflight(group)
        read_admitted=False
        for record in group:
            if record.get("kind")=="key-provider-preflight" and record.get("phase")=="read" and record.get("injected") is True:
                read_admitted=True
            if record.get("key_ready") is True and not read_admitted:
                raise RuntimeError("key readiness preceded provider preflight")
    if not all(any(v.get("key_ready") is True for v in group) for group in groups):
        raise RuntimeError("injected key not ready in each independent launch")
    if not any(v.get("status") == COPIED for v in records):
        raise RuntimeError("copy action did not complete in the receiving application")
    return KEY_LINE


def validate_pasteboard(records, expected_line):
    """Every p2-pasteboard receipt (one per Copy) must show the app read back exactly the key line."""
    receipts = [r for r in records if r.get("kind") == "p2-pasteboard"]
    if not receipts:
        raise RuntimeError("no pasteboard read-back receipt: the app did not read its pasteboard back after Copy")
    sha256, length = expected_pasteboard(expected_line)
    for receipt in receipts:
        if receipt.get("pasteboard_sha256") is None:
            raise RuntimeError("the app read no text back from the pasteboard after Copy")
        if (receipt.get("pasteboard_sha256"), receipt.get("pasteboard_bytes")) != (sha256, length):
            raise RuntimeError("pasteboard read back by the app differs from the formatted public key")
    print("P2 pasteboard read back by the app matches the restricted public key (sha256 %s, %d bytes, %d copies)"
          % (sha256, length, len(receipts)), flush=True)
    return len(receipts)


def terminate(device, runner=subprocess.run):
    """Stops the app before any of its files are read. Exit 3 is simctl's 'nothing to terminate'
    (the app already exited); any other failure means the app may still be writing, so it raises."""
    result = runner(["xcrun", "simctl", "terminate", device, BUNDLE], text=True, capture_output=True)
    print("P2 terminate before reading the app's files: exit", result.returncode, (result.stderr or "").strip(), flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError("could not terminate the app before reading its files (simctl exit %d)" % result.returncode)
    return result.returncode == 0


def read_after_terminate(path, start):
    """The records of path from this run (time >= start), read once after the app was terminated.
    Every line must be a complete JSON record: nothing may be appending any more, so a missing
    file or a partly written line is a failure by name, not something to wait for."""
    path = Path(path)
    if not path.is_file():
        raise RuntimeError(f"{path.name} does not exist after the app was terminated")
    lines = path.read_bytes().split(b"\n")
    if lines[-1]:
        raise RuntimeError(f"{path.name} ends in a partly written line after the app was terminated")
    return [r for r in (json.loads(line) for line in lines[:-1] if line) if r.get("time", 0) >= start]


def pbpaste(device, attempts=3, delay=5, check_output=subprocess.check_output, sleep=time.sleep):
    """The receiving Simulator's pasteboard, or None when simctl could not read it."""
    for attempt in range(attempts):
        try:
            return check_output(["xcrun", "simctl", "pbpaste", device], text=True, timeout=30)
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            print("P2 pbpaste attempt", attempt + 1, "failed:", error, flush=True)
            if attempt < attempts - 1:
                sleep(delay)
    return None


def pasteboard_aid(device, expected_line, hosted, read=None):
    """simctl pbpaste as a Mac mini aid to the app's read-back: skipped on a hosted runner, where it
    times out; elsewhere a read failure is logged, and text other than the key line still fails
    (the app's read-back said the line was there). Returns whether the read was made and matched."""
    if hosted:
        print("P2 pbpaste aid skipped on a hosted runner (exit 60 there, run 44151000668 job 134279069207); "
              "the app's read-back SHA-256 is the pasteboard check", flush=True)
        return False
    clipboard = (read or pbpaste)(device)
    if clipboard is None:
        print("P2 pbpaste aid: simctl could not read the pasteboard; the app's read-back SHA-256 is the check", flush=True)
        return False
    if clipboard != expected_line:
        raise RuntimeError("receiving Simulator pasteboard differs from the formatted public key")
    print("P2 pbpaste aid: receiving Simulator pasteboard exactly matches the restricted public key.", flush=True)
    return True


def negative_controls(records, expected_line):
    """The validators refuse this run's own receipts with one fact removed or altered."""
    other = expected_pasteboard("restrict ssh-ed25519 OTHER yebisu-injected-qa")[0]
    for corrupted, check, expected in [
        ([], validate_records, "missing independent process launch receipts"),
        ([{**v, "status": "ready"} if v.get("status") == COPIED else v for v in records], validate_records,
         "copy action did not complete in the receiving application"),
        ([v for v in records if v.get("kind") != "p2-pasteboard"], lambda r: validate_pasteboard(r, expected_line),
         "no pasteboard read-back receipt: the app did not read its pasteboard back after Copy"),
        ([{**v, "pasteboard_sha256": other} if v.get("kind") == "p2-pasteboard" else v for v in records],
         lambda r: validate_pasteboard(r, expected_line), "pasteboard read back by the app differs from the formatted public key"),
    ]:
        try:
            check(corrupted)
        except RuntimeError as error:
            if str(error) != expected:
                raise
            print("P2 negative control refused:", expected, flush=True)
        else:
            raise RuntimeError("negative control unexpectedly passed: " + expected)


def xcodebuild_test_command(spec, device, derived_data, result):
    sha256, length = expected_pasteboard()
    return ["xcodebuild", "test", "-project", str(spec / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
            "-destination", "id=" + device, "-derivedDataPath", str(derived_data),
            "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO",
            "-only-testing:SmokeOracle/SmokeQA/testP2KeysAndRestart"], {
                "TEST_RUNNER_P2_PASTEBOARD_SHA256": sha256, "TEST_RUNNER_P2_PASTEBOARD_BYTES": str(length)}


def main():
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    start = time.time()
    destination = root / "target/ios-p2" / time.strftime("%Y%m%d-%H%M%S")
    destination.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", capture_output=True)
    print(bundle.stderr, flush=True)
    app_path = bundle.stdout.strip().splitlines()[-1]
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
    run("xcrun", "simctl", "install", device, app_path)
    spec = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(spec / "project.yml"), "--project", str(spec))
    # Class D (issue #142 stage 4): a new Simulator can be missing from xcodebuild's list for a while.
    sim_ready.wait_for_destination(spec / "SmokeOracle.xcodeproj", "SmokeOracle", device)
    command, runner_environment = xcodebuild_test_command(spec, device, root / "target/ios-smoke/build", destination / "result.xcresult")
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, BUNDLE, "data"], text=True).strip())
    receipts = container / "Documents/p2-setup.jsonl"
    try:
        run(*command, env={**os.environ, **runner_environment})
    finally:
        terminate(device)
        # Kept whole, also when XCTest failed: the app's side of the run for the evidence upload.
        if receipts.is_file():
            shutil.copyfile(receipts, destination / "p2-setup.jsonl")
    records = read_after_terminate(receipts, start)
    (destination / "app.jsonl").write_text("".join(json.dumps(v) + "\n" for v in records))
    (destination / "provider-preflight.json").write_text(
        json.dumps([r for r in records if r.get("kind") == "key-provider-preflight"], indent=2))
    attachments = destination / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(destination / "result.xcresult"), "--output-path", str(attachments))
    screenshots = [a for test in json.loads((attachments / "manifest.json").read_text()) for a in test["attachments"]
                   if a["suggestedHumanReadableName"].startswith("p2-restarted")]
    if len(screenshots) != 1:
        raise RuntimeError("missing P2 foreground screenshot before XCTest teardown")
    shutil.copyfile(attachments / screenshots[0]["exportedFileName"], destination / "screenshot.png")
    # The first-run frame and sheet (SH1-157 L7) are #101419 and #1c1c22, not the old setup page color.
    run("python3", "scripts/ios/screenshot-color.py", str(destination / "screenshot.png"), "101419", "--min", "0.4")
    expected_line = validate_records(records)
    validate_pasteboard(records, expected_line)
    pasteboard_aid(device, expected_line, os.environ.get("GITHUB_ACTIONS") == "true")
    negative_controls(records, expected_line)
    print("P2 PASS: key block controls found by QA identifier; injected key read and copied after process restart; "
          "the app read the key line back from its pasteboard.")
    print("Injected provider only; Keychain persistence, Secure Enclave, real authentication and Local Network remain owner checks.")
    (root / "target/ios-p2/latest.json").write_text(json.dumps({"directory": str(destination)}) + "\n")
    print("P2 RECEIVING EVIDENCE", destination, flush=True)


if __name__ == "__main__":
    main()
