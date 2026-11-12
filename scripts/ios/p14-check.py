#!/usr/bin/env python3
"""Exclusive-lease P14 real-app wire/toast gate; never requests OS permission."""
import argparse
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
NAMES = ["p14-toast", "p14-focus", "p14-quiet", "p14-invalid", "p14-disconnect", "p14-stale-tap"]


def validate(records, messages, sent, attachments):
    steps = [r for r in records if r.get("kind") == "p14-step"]
    if [r.get("step") for r in steps] != [1, 2, 3, 4, 5]:
        raise ValueError("missing/duplicate actual app steps")
    if not all(r.get("sent") for r in steps[:4]):
        raise ValueError("fixture commands did not reach current client")
    commands = ["p14-live", "p14-quiet", "p14-invalid", "p14-disconnect"]
    if [r.get("command") for r in sent] != commands:
        raise ValueError("missing/duplicate sending peer receipts")
    for receipt, pane in zip(sent, ["w1:p3", "w1:p1", "missing-pane", "w1:p1"]):
        value = receipt.get("message", {}).get("SemanticNotification", {})
        if value.get("pane_id") != pane or value.get("sound") is not None:
            raise ValueError("wrong wire notification shape")
    inputs = [m["ClientShellPaneInput"] for m in messages if "ClientShellPaneInput" in m]
    if any(m.get("pane_id") != "w1:p1" for m in inputs):
        raise ValueError("wrong receiving command context")
    events = [e for m in messages if "ClientShellPaneInput" in m for e in m["ClientShellPaneInput"]["events"]]
    if events != [{"Paste": c} for c in commands]:
        raise ValueError("unexpected input/backlog or missing peer command")
    endpoint = [m["ClientShellEndpointRequest"] for m in messages if "ClientShellEndpointRequest" in m]
    if any(m.get("boot_id") != "boot-v1" for m in endpoint):
        raise ValueError("wrong focus boot context")
    requests = [json.loads(m["ClientShellEndpointRequest"]["request"]) for m in messages if "ClientShellEndpointRequest" in m]
    focus = [r for r in requests if r.get("method") == "pane.focus"]
    if len(requests) != 1 or len(focus) != 1 or focus[0].get("params", {}).get("pane_id") != "w1:p3":
        raise ValueError("tap must focus current target exactly once")
    paints = [r for r in records if r.get("kind") == "p14-paint"]
    shown = [r for r in paints if r.get("step") == 1 and r.get("toast") == "Agent needs attention"]
    if not shown or not all(r.get("boot") == "boot-v1" and r.get("host") == "qa@private-peer" and r.get("active") and r.get("connected") and r.get("generation") == steps[0].get("generation_after") and len(r.get("bounds", [])) == 4 and min(r["bounds"][2:]) > 0 for r in shown):
        raise ValueError("missing coherent actual toast paint")
    if not any(r.get("step") == 1 and r.get("pane") == "w1:p3" and r.get("toast") is None for r in paints):
        raise ValueError("missing receiving focus paint")
    for step in [2, 3, 4, 5]:
        phase = [r for r in paints if r.get("step") == step]
        if not phase or any(r.get("toast") is not None for r in phase):
            raise ValueError("negative phase displayed a toast or lacked paint")
    for step, reason in [(2, "QuietHours"), (3, "InvalidTarget"), (5, "Stale")]:
        if not any(r.get("step") == step and reason in r.get("status", "") for r in paints):
            raise ValueError("missing specific negative rejection: " + reason)
    if not any(r.get("step") == 4 and r.get("connected") is False and r.get("active") is False for r in paints):
        raise ValueError("disconnect did not reach consumer")
    late = steps[-1]
    if late.get("generation_before") != late.get("generation_after") or late.get("sent"):
        raise ValueError("late tap reconnected/replayed")
    if not all(name in attachments for name in NAMES):
        raise ValueError("missing actual XCTest screenshot attachments")
    auth = {r.get("authorization") for r in paints} - {None, "Unknown"}
    if not auth:
        raise ValueError("actual OS read-only authorization query not observed")
    return {"consumer_wire_toast_focus": "pass", "actual_authorization_query": sorted(auth),
            "os_banner": "UNVERIFIED", "sound": "UNVERIFIED", "physical": "UNVERIFIED",
            "permission_grant": "UNVERIFIED", "sdk_refusal_observer": "UNVERIFIED"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lease-id", required=True, help="controller's granted exclusive native lease")
    args = parser.parse_args()
    out = ROOT / "target/ios-p14" / time.strftime("%Y%m%d-%H%M%S")
    out.mkdir(parents=True)
    counter = 0

    def run(*argv):
        nonlocal counter
        counter += 1
        result = subprocess.run(argv, cwd=ROOT, text=True, capture_output=True)
        (out / f"command-{counter}.json").write_text(json.dumps({"argv": argv, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}))
        result.check_returncode()
        return result.stdout

    (out / "run.json").write_text(json.dumps({"lease": args.lease_id, "head": run("git", "rev-parse", "HEAD"), "native": "UNRUN until receipts"}))
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu").strip().splitlines()[-1]
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').split()[0]
    run("xcrun", "simctl", "install", device, bundle)
    container = Path(run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").strip())
    for name in ["p0-smoke.jsonl", "p4-peer.jsonl", "p14-sent.jsonl"]:
        path = container / "Documents" / name
        if path.exists():
            (out / ("previous-" + name)).write_bytes(path.read_bytes())
            path.unlink()
    # Only the isolated QA settings file; ordinary state is never reset.
    for path in container.rglob("p14-qa/yebisu-settings.json"):
        (out / "previous-qa-settings.json").write_bytes(path.read_bytes())
        path.unlink()
    project = ROOT / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    started = time.time()
    failure = None
    try:
        output = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-smoke/build"), "-resultBundlePath", str(out / "result.xcresult"), "CODE_SIGNING_ALLOWED=NO", "-only-testing:SmokeOracle/SmokeQA/testP14ConnectedNotifications")
        if "testP14ConnectedNotifications]' passed" not in output:
            raise ValueError("XCTest flow was not executed")
    except (subprocess.CalledProcessError, ValueError) as error:
        failure = error
    finally:
        # Preserve primary bytes even on failed XCTest or oracle; never promote models.
        for name in ["p0-smoke.jsonl", "p4-peer.jsonl", "p14-sent.jsonl"]:
            path = container / "Documents" / name
            if path.exists(): (out / name).write_bytes(path.read_bytes())
        run("xcrun", "xcresulttool", "export", "attachments", "--path", str(out / "result.xcresult"), "--output-path", str(out / "attachments"))
    if failure: raise failure
    read = lambda name: [json.loads(line) for line in (out / name).read_text().splitlines()]
    records = [r for r in read("p0-smoke.jsonl") if r.get("time", 0) >= started]
    manifest = json.loads((out / "attachments/manifest.json").read_text())
    names = {name for test in manifest for attachment in test["attachments"] for name in NAMES if attachment["suggestedHumanReadableName"].startswith(name) and (out / "attachments" / attachment["exportedFileName"]).is_file()}
    result = validate(records, read("p4-peer.jsonl"), read("p14-sent.jsonl"), names)
    (out / "receiving-result.json").write_text(json.dumps(result, indent=2))
    print(json.dumps(result))


if __name__ == "__main__":
    main()
