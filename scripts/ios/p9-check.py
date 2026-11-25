#!/usr/bin/env python3
"""Observe Herdr 2 through UIKit, a private core wire peer and app paint receipts."""
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
_live_spec = importlib.util.spec_from_file_location("live_jsonl", Path(__file__).with_name("live_jsonl.py"))
live_jsonl = importlib.util.module_from_spec(_live_spec)
_live_spec.loader.exec_module(live_jsonl)


def run(*args, **kwargs):
    print("P9 COMMAND", list(args), flush=True)
    if args[:2] == ("xcodebuild", "test"):
        # Issue #142 stage 4: streamed as before, and kept for the job's resend budget.
        # Class D: once more when xcodebuild found no device and ran no test (sim_ready.run_test).
        result = sim_ready.run_test(list(args), lambda: subprocess.CompletedProcess(list(args), *resend_budget.run_tee(list(args), **kwargs)))
        resend_budget.record(Path(__file__).stem, result.stdout)
        if result.returncode:
            raise subprocess.CalledProcessError(result.returncode, args, result.stdout)
        return result
    return subprocess.run(args, check=True, text=True, **kwargs)


def terminate(device):
    """Stops the app so none of its files is read while it may still write (UI test redesign S2:
    files are read only after terminate). Exit 3 is simctl's 'nothing to terminate'."""
    result = subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], text=True, capture_output=True)
    print("P9 terminate before reading the app's files: exit", result.returncode, (result.stderr or "").strip(), flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError("could not terminate the app before reading its files (simctl exit %d)" % result.returncode)


def validate(paints, wire):
    core = [p for p in paints if p.get("kind") == "terminal" and p.get("boot", "").startswith("core-")]
    generations = {p["generation"] for p in core}
    if len(generations) < 2:
        raise ValueError("no core frame after reattachment")
    by_terminal = {}
    for receipt in wire:
        if receipt.get("method") == "terminal.put" or is_doc(receipt):
            continue
        identity = receipt["terminal"]
        by_terminal.setdefault(identity, bytearray()).extend(receipt["bytes"])
    # Source-defined XCTest input and the peer's terminal identities; never
    # derive an expected value from pixels or from a previous successful run.
    first = [identity for identity, data in by_terminal.items() if b"P9-native" in data]
    second = [identity for identity, data in by_terminal.items() if b"P9-pane2" in data]
    if len(first) != 1 or len(second) != 1 or first[0] == second[0]:
        raise ValueError("split input did not reach two distinct terminal IDs")
    if b"P9-pane1" not in by_terminal[first[0]]:
        raise ValueError("reattachment did not preserve the first terminal ID")
    if "かな😀変換".encode() not in by_terminal[first[0]]:
        raise ValueError("IME commit did not reach the first terminal")
    print("P9 receiving-system PASS", "generations", sorted(generations),
          "terminals", {key: list(value) for key, value in by_terminal.items()}, flush=True)


def is_doc(receipt):
    """A herdr-app receipt of the mock core (#144): a doc.* call, or a terminal.run with the
    official app's labels. validate_doc reads those; the other observers skip them."""
    method = str(receipt.get("method", ""))
    return method.startswith("doc.") or method == "terminal.run"


# testP9HerdrDoc's writes (#144 D4): S5 saves one New tab; S3's New tab is refused once as stale
# and saved over the newer version; S1 and S4 write nothing. Then testP9HerdrDocRename's (D3,
# XCTest runs it after testP9HerdrDoc): S2 saves one rename; S3's rename is refused once as stale
# and saved over the newer version. Renames start no terminal.
DOC_SETS = ("ok", "conflict", "ok", "ok", "conflict", "ok")
LABELS = ("herdr.seen", "herdr.space", "herdr.tab")


def validate_doc(wire, expected=DOC_SETS, labelled=2):
    """#144: the doc.set receipts the mock core wrote, in order, and each terminal.run receipt's
    labels. A write refused as stale is sent again over a newer version, never the same bytes."""
    sets = [receipt for receipt in wire if receipt.get("method") == "doc.set"]
    for receipt in sets:
        if type(receipt.get("version")) is not int or type(receipt.get("bytes")) is not int:
            raise ValueError("malformed doc.set receipt: %r" % (receipt,))
    results = tuple(receipt.get("result") for receipt in sets)
    if results != tuple(expected):
        raise ValueError("expected doc.set results %r, found %r" % (tuple(expected), results))
    for refused, again in zip(sets, sets[1:]):
        if refused.get("result") == "conflict":
            if again["version"] <= refused["version"] or again["bytes"] == refused["bytes"]:
                raise ValueError("a stale write was sent again unchanged: %r %r" % (refused, again))
    runs = [receipt for receipt in wire if receipt.get("method") == "terminal.run"]
    for run in runs:
        labels = run.get("labels")
        if not isinstance(labels, dict) or tuple(sorted(labels)) != LABELS or labels["herdr.seen"] != "0":
            raise ValueError("a new terminal without the official labels: %r" % (run,))
    if len(runs) != labelled:
        raise ValueError("expected %d labelled terminal.run receipts, found %d" % (labelled, len(runs)))
    print("HERDR DOC receiving-system PASS: doc.set", list(results), "labelled terminals", len(runs), flush=True)


def validate_images(wire):
    pending = None
    puts = pasted = shadows = 0
    for receipt in wire:
        if is_doc(receipt):
            continue
        if receipt.get("method") == "terminal.put":
            if pending is not None or receipt.get("name") != "paste.png":
                raise ValueError("overlapping or malformed put")
            count = receipt.get("decoded")
            if type(count) is not int or not 0 < count <= 2 * 1024 * 1024:
                raise ValueError("invalid PNG byte count")
            terminal = receipt.get("terminal")
            if not isinstance(terminal, str) or not terminal:
                raise ValueError("missing put terminal")
            pending = (terminal, f"/state\\ with\\ space/paste-{puts}.png".encode())
            puts += 1
        elif "bytes" in receipt:
            raw = receipt["bytes"]
            if not isinstance(raw, list) or any(type(b) is not int or not 0 <= b <= 255 for b in raw):
                raise ValueError("invalid input bytes")
            data = bytes(raw)
            if data.startswith(b"/state"):
                if pending != (receipt.get("terminal"), data):
                    raise ValueError("path missing put, duplicated, malformed, or sent to wrong terminal")
                pending = None
                pasted += 1
            elif data == b"P8-shadow":
                if pending is not None:
                    raise ValueError("text before pending image")
                shadows += 1
        else:
            raise ValueError("unknown core receipt schema")
    if (puts, pasted, shadows, pending) != (3, 3, 1, None):
        raise ValueError(f"image receipts incomplete: puts={puts} paths={pasted} text={shadows}")
    print("SH1-154 receiving-system PASS: three PNG puts, three escaped paths, one text paste", flush=True)


def collect_evidence(directory, result, paints_path, wire_path):
    # Preserve app-side evidence before CI deletes its disposable Simulator,
    # including when XCTest fails before the validation/screenshot assertions.
    for source, name in [(paints_path, "paints.jsonl"), (wire_path, "wire.jsonl")]:
        if source.is_file():
            shutil.copyfile(source, directory / name)
    # Issue #142 stage 4 (classes Q and S): main-thread stalls in the app's own file, reported, not judged.
    try:
        gaps = live_jsonl.heartbeat_gaps(live_jsonl.read_records(directory / "paints.jsonl"))
        (directory / "heartbeat-gaps.json").write_text(json.dumps(gaps, indent=1) + "\n")
        print("P9 heartbeat gaps:", json.dumps(gaps), flush=True)
    except (OSError, ValueError) as error:
        print(f"P9 heartbeat gaps unreadable: {error}", flush=True)
    try:
        export = directory / "attachments"
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
        for test in json.loads((export / "manifest.json").read_text()):
            for attachment in test["attachments"]:
                if attachment["suggestedHumanReadableName"].startswith("p9-reattached"):
                    shutil.copyfile(export / attachment["exportedFileName"], directory / "screenshot.png")
                if attachment["suggestedHumanReadableName"].startswith("sh154-core-image"):
                    shutil.copyfile(export / attachment["exportedFileName"], directory / "sh154-core-image.png")
                if attachment["suggestedHumanReadableName"].startswith("p9-herdr-doc-rename"):
                    shutil.copyfile(export / attachment["exportedFileName"], directory / "herdr-doc-rename.png")
                elif attachment["suggestedHumanReadableName"].startswith("p9-herdr-doc"):
                    shutil.copyfile(export / attachment["exportedFileName"], directory / "herdr-doc.png")
    except subprocess.CalledProcessError as error:
        print(f"P9 attachment export failed: {error}", flush=True)


def xcodebuild_test_command(project, device, derived_data, result):
    # -test-timeouts-enabled makes XCTest enforce executionTimeAllowance (SmokeQA sets one
    # for testP9Core), so a hang fails that test in minutes instead of the 40-minute job limit.
    # Since S3 both tests press QA identifiers and wait on qa.state (YEBISU_QA_AX, set by the
    # tests' launchEnvironment); P9's start mode is its YEBISU_QA_START_SELECTION.
    return ["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
            "-test-timeouts-enabled", "YES",
            "-only-testing:SmokeOracle/SmokeQA/testP9Core()", "-only-testing:SmokeOracle/SmokeQA/testSH154CoreImage()",
            # #144: the herdr-app document read from the mock core (SmokeQA+HerdrDoc.swift).
            "-only-testing:SmokeOracle/SmokeQA/testP9HerdrDoc()",
            # #144 D3: Rename Space writes the document (SmokeQA+HerdrDoc.swift).
            "-only-testing:SmokeOracle/SmokeQA/testP9HerdrDocRename()",
            # The pure checks of QAAx.press's decisions (no app; under a second).
            "-only-testing:SmokeOracle/QAAxLogicTests",
            "-destination", "id=" + device, "-derivedDataPath", str(derived_data),
            "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO"]


def main():
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    directory = root / "target/ios-p9" / time.strftime("%Y%m%d-%H%M%S")
    directory.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", capture_output=True)
    print(bundle.stderr, flush=True)
    app = bundle.stdout.strip().splitlines()[-1]
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
    run("xcrun", "simctl", "install", device, app)
    terminate(device)  # an earlier check's app may still run; the baseline counts are read after it stops
    container = Path(subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip())
    wire_path = container / "Documents/p9-core-wire.jsonl"
    previous = len(wire_path.read_text().splitlines()) if wire_path.exists() else 0
    paints_path = container / "Documents/p0-smoke.jsonl"
    previous_paints = len(paints_path.read_text().splitlines()) if paints_path.exists() else 0
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    # Class D (issue #142 stage 4): a new Simulator can be missing from xcodebuild's list for a while.
    sim_ready.wait_for_destination(project / "SmokeOracle.xcodeproj", "SmokeOracle", device)
    result = directory / "result.xcresult"
    try:
        run(*xcodebuild_test_command(project, device, root / "target/ios-smoke/build", result))
    finally:
        # The app is stopped before its receipts are copied or read, also when XCTest failed.
        terminate(device)
        collect_evidence(directory, result, paints_path, wire_path)
    if not (directory / "screenshot.png").is_file():
        raise ValueError("XCTest did not execute the P9 screenshot step")
    if not (directory / "sh154-core-image.png").is_file():
        raise ValueError("XCTest did not execute SH1-154 screenshot step")
    if not (directory / "herdr-doc.png").is_file():
        raise ValueError("XCTest did not execute the herdr-app document screenshot step")
    if not (directory / "herdr-doc-rename.png").is_file():
        raise ValueError("XCTest did not execute the herdr-app rename screenshot step")
    paints = [json.loads(line) for line in paints_path.read_text().splitlines()[previous_paints:]]
    wire = [json.loads(line) for line in wire_path.read_text().splitlines()[previous:]]
    validate(paints, wire)
    validate_images(wire)
    validate_doc(wire)
    (directory / "receipts.json").write_text(json.dumps({"paints": paints, "wire": wire}))
    print("P9 screenshot", directory / "screenshot.png", flush=True)


if __name__ == "__main__":
    main()
