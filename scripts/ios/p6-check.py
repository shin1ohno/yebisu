#!/usr/bin/env python3
"""Exercise the snapshot board and semantic navigation using private peers."""
import json
import shutil
import subprocess
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
    tested = run("xcodebuild", "clean", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-only-testing:SmokeOracle/SmokeQA/testP6Board", "-destination", "id=" + device, "-derivedDataPath", str(out / "build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
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
    peer = Path(container) / "Documents/p4-peer.jsonl"
    messages = [json.loads(line) for line in peer.read_text().splitlines()]
    methods = [json.loads(m["ClientShellEndpointRequest"]["request"])["method"] for m in messages if "ClientShellEndpointRequest" in m]
    if not {"workspace.focus", "pane.focus"}.issubset(methods):
        raise ValueError("board taps did not reach the peer API: " + repr(methods))
    print("P6 PASS: board taps reached workspace.focus and pane.focus on private peer")

if __name__ == "__main__":
    main()
