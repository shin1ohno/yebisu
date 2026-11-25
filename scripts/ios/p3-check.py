#!/usr/bin/env python3
"""Run the private wire peer and verify terminal receipts and source-defined colours."""
import argparse
import importlib.util
import json
import os
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

_live_spec = importlib.util.spec_from_file_location("live_jsonl", Path(__file__).with_name("live_jsonl.py"))
live_jsonl = importlib.util.module_from_spec(_live_spec)
_live_spec.loader.exec_module(live_jsonl)


def run(*args, **kwargs):
    print("P3 COMMAND", list(args), flush=True)
    if args[:2] == ("xcodebuild", "test"):
        # Issue #142 stage 4: streamed as before, and kept for the job's resend budget.
        # Class D: once more when xcodebuild found no device and ran no test (sim_ready.run_test).
        result = sim_ready.run_test(list(args), lambda: subprocess.CompletedProcess(list(args), *resend_budget.run_tee(list(args), **kwargs)))
        resend_budget.record(Path(__file__).stem, result.stdout)
        if result.returncode:
            raise subprocess.CalledProcessError(result.returncode, args, result.stdout)
        return result
    try:
        return subprocess.run(args, check=True, text=True, **kwargs)
    except subprocess.CalledProcessError as error:
        print(error.stdout or "", flush=True)
        print(error.stderr or "", flush=True)
        raise


def terminate(device):
    """Stops the app so none of its files is read while it may still write (UI test redesign S2:
    files are read only after terminate). Exit 3 is simctl's 'nothing to terminate'."""
    result = subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], text=True, capture_output=True)
    print("P3 terminate before reading the app's files: exit", result.returncode, (result.stderr or "").strip(), flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError("could not terminate the app before reading its files (simctl exit %d)" % result.returncode)


def validate_records(records, require_rotation=False):
    paints = [r for r in records if r.get("kind") == "terminal"]
    if not paints or any(r["boot"] != "boot-v1" or r["projection"] != 7 for r in paints):
        raise ValueError("missing or incoherent wire-peer paint receipts")
    if {r["compact"] for r in paints} != {False, True}:
        raise ValueError("both density controls must actually paint")
    sizes = {(r["cols"], r["rows"]) for r in paints if not r["compact"]}
    if len(sizes) < 2 or max(r["surface"] for r in paints) < 4:
        raise ValueError("peer did not receive and answer rotation/density geometry")
    rotated = any(r["viewport"][0] > r["viewport"][1] for r in paints)
    if require_rotation and not rotated:
        raise ValueError("peer did not receive and answer rotation/density geometry")
    print("P3 rotation:", "observed" if rotated else "UNVERIFIED: orientation request did not change the app viewport; owner device check required", flush=True)
    print("P3 receipts", len(paints), "standard geometries", sorted(sizes), flush=True)


def validate_colors(counts):
    # theme.rs ANSI palette, mock_peer.rs truecolour glyphs. Never sampled
    # from a screenshot to define an expected value. Each is a four-cell strip.
    for color in ("bf616a", "a3be8c", "ebcb8b", "81a1c1", "b48ead", "88c0d0", "00d8de"):
        if counts.get(bytes.fromhex(color), 0) < 100:
            raise ValueError("missing terminal colour/glyph region: " + color)


def validate(directory, require_rotation=False):
    records = live_jsonl.read_records(directory / "app.jsonl")
    validate_records(records, require_rotation)
    spec = importlib.util.spec_from_file_location("colors", Path(__file__).with_name("screenshot-color.py"))
    colors = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(colors)
    for stage in ("p3-standard", "p3-compact", "p3-landscape", "p3-returned"):
        width, height, channels, rows = colors.decode((directory / (stage + ".png")).read_bytes())
        counts = colors.color_counts(rows, channels)
        validate_colors(counts)
        print("P3 source-colour oracle PASS", stage, width, height, flush=True)


def collect(result, directory, device, require_rotation=False):
    export = directory / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("p3-"):
                shutil.copyfile(export / attachment["exportedFileName"], directory / (stage + ".png"))
    container = subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip()
    shutil.copyfile(Path(container) / "Documents/p0-smoke.jsonl", directory / "app.jsonl")
    validate(directory, require_rotation)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-rotation", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    directory = root / "target/ios-p3" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    app = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", capture_output=True)
    print(app.stderr, flush=True)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
    run("xcrun", "simctl", "install", device, app.stdout.strip().splitlines()[-1])
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    # Class D (issue #142 stage 4): a new Simulator can be missing from xcodebuild's list for a while.
    sim_ready.wait_for_destination(project / "SmokeOracle.xcodeproj", "SmokeOracle", device)
    result = directory / "result.xcresult"
    try:
        tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-only-testing:SmokeOracle/SmokeQA/testP3Terminal", "-destination", "id=" + device, "-derivedDataPath", str(root / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-skip-testing:SmokeOracle/SmokeQA/testInputAndLifecycleReceipts", "-skip-testing:SmokeOracle/SmokeQA/testP2KeysAndRestart", capture_output=True)
    finally:
        # Stopped before p0-smoke.jsonl is copied, also when XCTest failed.
        terminate(device)
    print(tested.stdout, flush=True)
    print(tested.stderr, flush=True)
    # Xcode can exit 0 with an unmatched test filter: require execution too.
    if "testP3Terminal]' passed" not in tested.stdout:
        raise RuntimeError("terminal XCTest did not execute and pass")
    collect(result, directory, device, args.require_rotation)
    print("P3 receiving-system evidence", directory, flush=True)


if __name__ == "__main__":
    main()
