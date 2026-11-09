#!/usr/bin/env python3
"""Exercise the snapshot board and semantic navigation using private peers."""
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def validate(records, messages):
    requests = [json.loads(m["ClientShellEndpointRequest"]["request"]) for m in messages if "ClientShellEndpointRequest" in m]
    if [r["method"] for r in requests] != ["tab.create", "pane.split", "tab.focus"]:
        raise ValueError("incorrect API sequence")
    if requests[0]["params"].get("workspace_id") != "w1" or requests[1]["params"].get("direction") != "right" or requests[2]["params"].get("tab_id") != "w1:t1":
        raise ValueError("incorrect API targets")
    if any("ClientShellPaneInput" in m for m in messages):
        raise ValueError("actions synthesized terminal input")
    paints = [r for r in records if r.get("kind") == "terminal" and r.get("boot") == "boot-v1"]
    # Mock source starts with one w1 tab and two panes; creating a tab adds one pane.
    expected = [("w1:t1", "w1:p1", 1, 2), ("w1:t3", "w1:p13", 2, 1), ("w1:t3", "w1:p14", 2, 2), ("w1:t1", "w1:p1", 2, 2)]
    cursor = 0
    for target in expected:
        for i in range(cursor, len(paints)):
            r = paints[i]
            if (r.get("tab"), r.get("pane"), r.get("tab_count"), r.get("pane_count")) == target and r.get("input_pane") == target[1]:
                cursor = i + 1
                break
        else:
            raise ValueError("missing coherent tab/pane/input transition: " + repr(target))
    print("P7 PASS: tab.create, pane.split, tab.focus received; tab/pane counts, displayed focus and input destination changed in order; no synthesized input")


def main():
    root = Path(__file__).resolve().parents[2]
    out = root / "target/ios-p7"
    out.mkdir(exist_ok=True)
    def run(*args):
        print("P7 COMMAND", args, flush=True)
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
        tested = run("xcodebuild", "test", "-sdk", "iphonesimulator", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-only-testing:SmokeOracle/SmokeQA/testP7Tabs", "-destination", "id=" + device, "-derivedDataPath", build, "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
    if "testP7Tabs]' passed" not in tested:
        raise RuntimeError("P7 XCTest did not execute")
    export = out / "attachments"
    if export.exists():
        shutil.rmtree(export)
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("p7-"):
                shutil.copyfile(export / attachment["exportedFileName"], out / (stage + ".png"))
    container = run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").strip()
    shutil.copyfile(Path(container) / "Documents/p0-smoke.jsonl", out / "app.jsonl")
    records = [json.loads(line) for line in (out / "app.jsonl").read_text().splitlines()]
    peer = Path(container) / "Documents/p4-peer.jsonl"
    messages = [json.loads(line) for line in peer.read_text().splitlines()]
    print("P7 sample paint", next(r for r in records if r.get("kind") == "terminal"))
    print("P7 sample wire", next(m for m in messages if "ClientShellEndpointRequest" in m))
    validate(records, messages)

if __name__ == "__main__":
    main()
