#!/usr/bin/env python3
"""Run native menu cases and read actual clipboard, wire and UIKit receipts."""
import importlib.util
import json
import re
from pathlib import Path
import shutil
import subprocess
import time

_ready_spec = importlib.util.spec_from_file_location("sim_ready", Path(__file__).with_name("sim_ready.py"))
sim_ready = importlib.util.module_from_spec(_ready_spec)
_ready_spec.loader.exec_module(sim_ready)
_budget_spec = importlib.util.spec_from_file_location("resend_budget", Path(__file__).with_name("resend_budget.py"))
resend_budget = importlib.util.module_from_spec(_budget_spec)
_budget_spec.loader.exec_module(resend_budget)

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)
transition_spec = importlib.util.spec_from_file_location("sh1_152_transitions", Path(__file__).with_name("sh1_152_transitions.py"))
transitions = importlib.util.module_from_spec(transition_spec)
transition_spec.loader.exec_module(transitions)
scene_spec = importlib.util.spec_from_file_location("sh1_152_scenes", Path(__file__).with_name("sh1_152_scenes.py"))
scenes = importlib.util.module_from_spec(scene_spec)
scene_spec.loader.exec_module(scenes)
_live_spec = importlib.util.spec_from_file_location("live_jsonl", Path(__file__).with_name("live_jsonl.py"))
live_jsonl = importlib.util.module_from_spec(_live_spec)
_live_spec.loader.exec_module(live_jsonl)
_stderr_spec = importlib.util.spec_from_file_location("app_stderr", Path(__file__).with_name("app_stderr.py"))
app_stderr = importlib.util.module_from_spec(_stderr_spec)
_stderr_spec.loader.exec_module(app_stderr)


def run(*args, **kwargs):
    print("SH1-152 COMMAND", list(args), flush=True)
    if args[:2] == ("xcodebuild", "test"):
        # Issue #142 stage 4 (class D): once more when xcodebuild found no device and ran no test.
        result = sim_ready.run_test(args, lambda: subprocess.run(args, cwd=ROOT, text=True, capture_output=True, **kwargs))
    else:
        result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, **kwargs)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    if args[:2] == ("xcodebuild", "test"):
        resend_budget.record(Path(__file__).stem, result.stdout)  # issue #142 stage 4: the job's resend budget
    result.check_returncode()
    return result


def terminate(device):
    """Stops the app so none of its files is read while it may still write (UI test redesign S4:
    EditMenuQA waits on qa.state, and the files are read only after terminate). Exit 3 is simctl's
    'nothing to terminate'."""
    result = subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], text=True, capture_output=True)
    print("SH1-152 terminate before reading the app's files: exit", result.returncode, (result.stderr or "").strip(), flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError("could not terminate the app before reading its files (simctl exit %d)" % result.returncode)


def main():
    directory = ROOT / "target/ios-sh1-152" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu").stdout.strip().splitlines()[-1]
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').stdout.split()[0]
    run("xcrun", "simctl", "install", device, bundle)
    terminate(device)  # an earlier check's app may still run and write the files removed here
    container = Path(run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").stdout.strip()) / "Documents"
    for path in [*container.glob("sh1-152-*.jsonl"), container / "p4-peer.jsonl", container / "p9-core-wire.jsonl"]:
        path.unlink(missing_ok=True)
    stderr_start = app_stderr.offset(container)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    # Class D (issue #142 stage 4): a new Simulator can be missing from xcodebuild's list for a while.
    sim_ready.wait_for_destination(project / "SmokeOracle.xcodeproj", "SmokeOracle", device)
    methods = ["testSH1152Basic", "testSH1152Transitions", "testSH1152DismissAndSystemEdit", "testSH1152TwoScenes"]
    try:
        # A failing XCTest exits non-zero; every receiving-system oracle still runs below.
        print("SH1-152 COMMAND xcodebuild test", flush=True)
        result = sim_ready.run_test(xctest_command := ["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device,
                                 "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(directory / "result.xcresult"),
                                 "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/EditMenuQA"], lambda: subprocess.run(xctest_command, cwd=ROOT, text=True, capture_output=True))
        resend_budget.record("sh1-152-check", result.stdout)
        print(result.stdout, flush=True)
        print(result.stderr, flush=True)
        (directory / "xctest.log").write_text(result.stdout + result.stderr)
    finally:
        # The app is stopped before its receipts are copied or read, also when XCTest failed.
        terminate(device)
        for path in [*container.glob("sh1-152-*.jsonl"), container / "p4-peer.jsonl", container / "p9-core-wire.jsonl"]:
            if path.exists():
                shutil.copyfile(path, directory / path.name)
    read = lambda path: live_jsonl.read_records(path) if path.exists() else []
    try:
        records = [row for path in directory.glob("sh1-152-*.jsonl") for row in read(path)]
        peer, core = read(directory / "p4-peer.jsonl"), read(directory / "p9-core-wire.jsonl")
        gap = None
    except ValueError as error:
        # An observation-dropped record (live_jsonl.reject_gaps): every oracle below
        # would read an incomplete file, so the receipts check reports why.
        records, peer, core, gap = [], [], [], error
    verdicts, failures = {"xctest": {m: f"{m}]' passed" in result.stdout for m in methods}}, []
    failures += [f"XCTest did not execute and pass: {m}" for m, ok in verdicts["xctest"].items() if not ok]
    # Issue #142 stage 4 (classes Q and S): where the app's main thread wrote no heartbeat, per case file.
    # Reported, not judged.
    verdicts["heartbeat_gaps"] = heartbeat_report(directory)

    def receipts():
        app_stderr.require_no_loss(app_stderr.collect(container, directory, stderr_start))
        if gap is not None:
            raise gap
        return "SH1-152 receipts complete: no lost-record line or record"

    def launches():
        return launch_retries_proven(result.stdout, app_stderr.collect(container, directory, stderr_start))

    checks = {
        "receipts": receipts,
        "launch_retries": launches,
        "basic": lambda: oracle.validate_basic(records, peer, core),
        "menus": lambda: oracle.visible_menus(result.stdout, [*oracle.BASIC_CASES, *transitions.CASES, "E4-text", "E4-key", "E7", *scenes.CASES]),
        "transitions": lambda: transitions.validate(records, peer, core, result.stdout),
        "scenes": lambda: scenes.validate(records, peer),
    }
    for name, check in checks.items():
        try:
            verdicts[name] = check()
        except (ValueError, KeyError, TypeError, IndexError) as error:
            verdicts[name] = f"FAIL: {type(error).__name__}: {error}"
            failures.append(f"{name}: {error}")
    (directory / "verdict.json").write_text(json.dumps(verdicts, ensure_ascii=False, indent=1))
    print("SH1-152 VERDICT", json.dumps(verdicts, ensure_ascii=False), directory, flush=True)
    if failures:
        raise SystemExit("SH1-152 FAIL\n" + "\n".join(failures))

def launch_retries_proven(xctest_output, stderr_text):
    """A launch-retry (EditMenuQA, class L) is proven only when the failed launch started no process:
    every process start writes one `app-stderr launch pid=` line (crates/yebisu/src/qa_stderr.rs), and
    every attempt prints `SH1-152 LAUNCH <case> <seconds>`, so starts = attempts - launch retries."""
    retries = [r for r in resend_budget.parse(xctest_output) if r.get("kind") == "launch-retry"]
    if not retries:
        return "no launch-retry"
    attempts = len(re.findall(r"^SH1-152 LAUNCH \S+ [0-9.]+$", xctest_output, re.M))
    starts = stderr_text.count("app-stderr launch pid=")
    if starts != attempts - len(retries):
        raise ValueError(f"launch-retry not proven: {starts} app starts for {attempts} launch attempts and "
                         f"{len(retries)} launch retries ({[r.get('id') for r in retries]})")
    return f"{len(retries)} launch-retry proven: {starts} app starts for {attempts} attempts"


def heartbeat_report(directory):
    """live_jsonl.heartbeat_gaps of each sh1-152-*.jsonl, with its file name; a file that cannot be read is named."""
    report = []
    for path in sorted(Path(directory).glob("sh1-152-*.jsonl")):
        try:
            report += [dict(gap, file=path.name) for gap in live_jsonl.heartbeat_gaps(live_jsonl.read_records(path))]
        except ValueError as error:
            report.append({"file": path.name, "unreadable": str(error)})
    return report


if __name__ == "__main__":
    main()
