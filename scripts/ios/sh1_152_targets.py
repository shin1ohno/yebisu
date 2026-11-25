#!/usr/bin/env python3
"""L61: where a pointer's secondary click shows the native edit menu.

Runs EditMenuTargetsQA on the Simulator the edit-menu job already booted, then
judges its `L61 TARGET {json}` lines: Paste on the terminal's content area only,
none on the sidebar's host, space and AGENTS rows, the rail, the tab row or the
status bar, and the long-press menus of a host row and a rail item without an
edit menu. Screenshots go to target/ios-sh1-152/l61-<time>/attachments, which
the job uploads. Simulator evidence, not physical pointer QA."""
import json
from pathlib import Path
import subprocess
import sys
import time
import importlib.util

_ready_spec = importlib.util.spec_from_file_location("sim_ready", Path(__file__).with_name("sim_ready.py"))
sim_ready = importlib.util.module_from_spec(_ready_spec)
_ready_spec.loader.exec_module(sim_ready)
_budget_spec = importlib.util.spec_from_file_location("resend_budget", Path(__file__).with_name("resend_budget.py"))
resend_budget = importlib.util.module_from_spec(_budget_spec)
_budget_spec.loader.exec_module(resend_budget)

ROOT = Path(__file__).resolve().parents[2]
TEST = "testL61SecondaryClickTargets"
PASTE = ("terminal", "terminal-again")
NO_MENU = ("agents-row", "space-row", "host-row", "tab-row", "status-bar", "rail", "rail-item")
LONG_PRESS = ("host-row-long-press", "rail-item-long-press")


def targets(log):
    """The TARGET records the XCTest printed, by name (a repeated name fails)."""
    found = {}
    for line in log.splitlines():
        marker = line.find("L61 TARGET ")
        if marker < 0:
            continue
        record = json.loads(line[marker + len("L61 TARGET "):])
        if record["name"] in found:
            raise ValueError(f"target {record['name']} reported twice")
        found[record["name"]] = record
    return found


def validate(log):
    """Returns the verdict per target; raises ValueError naming every failed target."""
    found = targets(log)
    failures, verdict = [], {}
    for name in (*PASTE, *NO_MENU, *LONG_PRESS):
        record = found.get(name)
        if record is None:
            problems = ["not clicked"]
        else:
            want = name in PASTE
            problems = [] if record.get("paste") is want else [
                f"Paste shown {record.get('paste')}, expected {want}, items {record.get('items')}"]
            if name == "host-row-long-press" and record.get("row_menu") is not True:
                problems.append("the row's long-press menu did not open")
            # The app's own receipts say where a missing or extra menu came from.
            if name in PASTE and record.get("presented", 0) < 1:
                problems.append(f"the app presented no menu (secondary clicks it saw: {record.get('secondary')})")
            if name in NO_MENU and record.get("presented", 0) > 0:
                problems.append("the app presented its menu outside the terminal")
        verdict[name] = "fail" if problems else "pass"
        failures += [f"{name}: {problem}" for problem in problems]
    if f"{TEST}]' passed" not in log:
        failures.append(f"XCTest {TEST} did not pass")
    if failures:
        raise ValueError("; ".join(failures))
    return verdict


def run(*args):
    print("L61 COMMAND", list(args), flush=True)
    if args[:2] == ("xcodebuild", "test"):
        # Issue #142 stage 4 (class D): once more when xcodebuild found no device and ran no test.
        result = sim_ready.run_test(args, lambda: subprocess.run(args, cwd=ROOT, text=True, capture_output=True))
    else:
        result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True)
    print(result.stdout, flush=True)
    print(result.stderr, flush=True)
    if args[:2] == ("xcodebuild", "test"):
        resend_budget.record(Path(__file__).stem, result.stdout)  # issue #142 stage 4: the job's resend budget
    result.check_returncode()
    return result


def main():
    directory = ROOT / "target/ios-sh1-152" / ("l61-" + time.strftime("%Y%m%d-%H%M%S"))
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu").stdout.strip().splitlines()[-1]
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').stdout.split()[0]
    run("xcrun", "simctl", "install", device, bundle)
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    # Class D (issue #142 stage 4): a new Simulator can be missing from xcodebuild's list for a while.
    sim_ready.wait_for_destination(project / "SmokeOracle.xcodeproj", "SmokeOracle", device)
    result_bundle = directory / "result.xcresult"
    print("L61 COMMAND xcodebuild test", flush=True)
    result = sim_ready.run_test(xctest_command := ["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
                             "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"),
                             "-resultBundlePath", str(result_bundle), "CODE_SIGNING_ALLOWED=NO",
                             "-only-testing:SmokeOracle/EditMenuTargetsQA/" + TEST], lambda: subprocess.run(xctest_command, cwd=ROOT, text=True, capture_output=True))
    resend_budget.record("sh1_152_targets", result.stdout)
    log = result.stdout + result.stderr
    print(log, flush=True)
    (directory / "xctest.log").write_text(log)
    subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], check=False)
    if result_bundle.exists():
        subprocess.run(["xcrun", "xcresulttool", "export", "attachments", "--path", str(result_bundle),
                        "--output-path", str(directory / "attachments")], check=False)
    try:
        verdict = validate(log)
    except (ValueError, KeyError) as error:
        (directory / "verdict.json").write_text(json.dumps({"fail": str(error)}, indent=1))
        sys.exit(f"L61 FAIL {error} ({directory})")
    (directory / "verdict.json").write_text(json.dumps(verdict, indent=1))
    print("L61 PASS", json.dumps(verdict), directory, flush=True)


if __name__ == "__main__":
    main()
