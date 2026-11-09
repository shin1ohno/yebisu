#!/usr/bin/env python3
"""Exercise the snapshot board and semantic navigation using private peers."""
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def validate(records):
    boards = [r for r in records if r.get("kind") == "board"]
    for host in ["qa-alpha", "qa-beta"]:
        seen = [r for r in boards if r["host"] == host]
        if len({r["revision"] for r in seen}) < 3:
            raise ValueError("host did not update live: " + host)
        hidden = [r for r in seen if not r["selected"]]
        if len({r["revision"] for r in hidden}) < 2:
            raise ValueError("hidden host did not update: " + host)
        if not any("Working" in r["states"] for r in seen) or not any("Blocked" in r["states"] for r in seen):
            raise ValueError("daemon state transitions missing: " + host)
        if not any(any("[Claude]" in row for row in r["rows"]) for r in seen):
            raise ValueError("harness mark missing")
    print("P6 PASS: both hosts live, hidden hosts update, daemon states and harness marks")


def validate_navigation(records):
    moves = [(i, r) for i, r in enumerate(records) if r.get("kind") == "navigation"]
    if len(moves) < 2:
        raise ValueError("navigation evidence missing")
    # These are the two targets selected by testP6Board from the peer fixture.
    if [move["pane"] for _, move in moves] != ["w2:p2", "w1:p1"]:
        raise ValueError("navigation target sequence differs from source-defined QA steps")
    for i, move in moves:
        expected = move["pane"]
        marker = "2" if expected == "w2:p2" else "1"
        before = [r for r in records[:i] if r.get("kind") == "terminal" and r.get("host") == move["host"]]
        if not before or before[-1].get("pane") == expected:
            raise ValueError("navigation did not select a different target")
        after = [r for r in records[i+1:] if r.get("kind") == "terminal"
                 and r.get("host") == move["host"] and r.get("boot") == move["boot"]
                 and r.get("generation") == move["generation"] and r.get("projection", 0) > move["revision"]
                 and r.get("pane") == expected and r.get("surface_marker") == marker and r.get("input_pane") == expected]
        if not after:
            raise ValueError("focus/surface/input destination did not move on current boot")
    print("P6 PASS: received focus, rendered surface and input destination changed for both moves")


def main():
    root = Path(__file__).resolve().parents[2]
    out = root / "target/ios-p6"
    out.mkdir(exist_ok=True)
    def run(*args):
        print("P6 COMMAND", args, flush=True)
        result = subprocess.run(args, cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(result.stdout, flush=True)
        result.check_returncode()
        return result.stdout
    built = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu")
    app = built.strip().splitlines()[-1]
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').strip().splitlines()[-1]
    run("xcrun", "simctl", "install", device, app)
    container = run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").strip()
    (Path(container) / "Documents/p4-peer.jsonl").unlink(missing_ok=True)
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = out / "result.xcresult"
    if result.exists():
        shutil.rmtree(result)
    # Isolate XCTest discovery from previous runs. Combined clean/test failed
    # destination resolution for this test-only scheme; scope cleanup here.
    with tempfile.TemporaryDirectory(prefix="runner-", dir=out) as build:
        tested = run("xcodebuild", "test", "-sdk", "iphonesimulator", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-only-testing:SmokeOracle/SmokeQA/testP6Board", "-destination", "id=" + device, "-derivedDataPath", build, "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
    if "testP6Board]' passed" not in tested:
        raise RuntimeError("P6 XCTest did not execute")
    export = out / "attachments"
    if export.exists():
        shutil.rmtree(export)
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("p6-"):
                shutil.copyfile(export / attachment["exportedFileName"], out / (stage + ".png"))
    container = run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").strip()
    shutil.copyfile(Path(container) / "Documents/p0-smoke.jsonl", out / "app.jsonl")
    records = [json.loads(line) for line in (out / "app.jsonl").read_text().splitlines()]
    validate(records)
    validate_navigation(records)
    peer = Path(container) / "Documents/p4-peer.jsonl"
    messages = [json.loads(line) for line in peer.read_text().splitlines()]
    requests = [json.loads(m["ClientShellEndpointRequest"]["request"]) for m in messages if "ClientShellEndpointRequest" in m]
    focus = [r for r in requests if r["method"] in {"workspace.focus", "pane.focus"}]
    if [r["method"] for r in focus] != ["workspace.focus", "pane.focus"] or focus[0]["params"].get("workspace_id") != "w2" or focus[1]["params"].get("pane_id") != "w1:p1":
        raise ValueError("board taps sent incorrect methods or targets: " + repr(focus))
    print("P6 PASS: board taps reached workspace.focus and pane.focus on private peer")

if __name__ == "__main__":
    main()
