#!/usr/bin/env python3
"""Lease-only native production-sheet probe. Injected keys and private wire peer."""
import json
import importlib.util
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]

def validate(records, messages, disk):
    starts=[r for r in records if r.get("kind")=="p11-probe-start"]
    ends=[r for r in records if r.get("kind")=="p11-probe-end"]
    if len(starts)!=1 or len(ends)!=1 or not starts[0].get("scheduled") or not starts[0].get("action_blocked") or not ends[0].get("modal"):
        raise ValueError("missing actual modal callback probe")
    actions=[r.get("action") for r in records if r.get("kind")=="p11-native-blocked"]
    if actions != ["Copy", "Paste", "Cut", "SelectAll"]:
        raise ValueError("native selectors: SDK entry/refusal is unobservable with pinned SDK; consumer receipts are not a substitute for a refusal hook")
    events=[]
    for message in messages:
        if "ClientShellPaneInput" in message:
            body=message["ClientShellPaneInput"]
            if body["pane_id"]!="w1:p1":raise ValueError("wrong receiving pane")
            events.extend(body["events"])
    if events!=[{"Paste":"p11-start"},{"Paste":"p11-end"}]:
        raise ValueError("modal wire leak or missing FIFO markers")
    paints=[r for r in records if r.get("kind")=="terminal" and r.get("font_size")==15]
    if not paints or disk.get("terminal_font_size")!=15:
        raise ValueError("font consumer/durable state mismatch")
    loaded=[r for r in records if r.get("kind")=="settings-loaded"]
    if len(loaded)<2 or loaded[-1].get("font_size")!=15:
        raise ValueError("relaunch did not reload the font")
    # A receiver resize must accompany the changed font, not just a setting receipt.
    if not any([message["ClientShellResize"]["cell_width_px"],message["ClientShellResize"]["cell_height_px"]] == paint.get("cell_size")
               for message in messages if "ClientShellResize" in message for paint in paints):
        raise ValueError("missing receiving resize")
    if any("key_line" in r for r in records):raise ValueError("key leaked into observation")

def validate_positive(records, messages):
    # Reuse the real-selector suite, including async paste completion and every
    # receiving FIFO segment. This launch is distinct from the modal launch.
    markers=[e["Paste"] for m in messages if "ClientShellPaneInput" in m
             for e in m["ClientShellPaneInput"]["events"] if "Paste" in e
             and e["Paste"].startswith(("native-start-", "native-end-"))]
    if markers != [f"native-{edge}-{i}" for i in range(24) for edge in ("start", "end")]:
        raise ValueError("outside-modal FIFO marker sequence mismatch")
    spec=importlib.util.spec_from_file_location("p8_native", Path(__file__).with_name("p8-native-check.py"))
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    module.validate(records, messages)
    # The terminal publishes select_all=false (SH1-153, terminal_controls.rs), so
    # the SDK refuses the scheduled selectAll: before any KeyDown and the consumer
    # never sees it (docs/p11-sdk11-adoption.md). A SelectAll receipt here would
    # mean the SDK dispatched a refused action. Physical C/V carry no native action.
    receipts=[r for r in records if r.get("kind")=="native-consumer"]
    if [r.get("action") for r in receipts] != ["Paste", "Copy", "Cut"]*4 or any(r.get("modal") is not False for r in receipts):
        raise ValueError("outside-modal real-selector consumer receipts missing or duplicated")
    if any(m["ClientShellPaneInput"].get("pane_id") != "w1:p1" for m in messages if "ClientShellPaneInput" in m):
        raise ValueError("outside-modal wrong receiving pane")

def observations(records, messages, disk):
    """Keep independently observed facts even when the native gate stays closed."""
    events=[e for m in messages if "ClientShellPaneInput" in m for e in m["ClientShellPaneInput"]["events"]]
    return {
        "modal_consumer_actions":[r.get("action") for r in records if r.get("kind")=="p11-native-blocked"],
        "sdk_refusal_observable":False,
        "sdk_gate":"unsupported: separate reviewed SDK observer dependency required",
        "fifo_markers_only":events==[{"Paste":"p11-start"},{"Paste":"p11-end"}],
        "durable_font_size":disk.get("terminal_font_size"),
    }

def terminate(device):
    """Stops the app so none of its files is read while it may still write (UI test redesign S2:
    files are read only after terminate; S6 for P11). Exit 3 is simctl's 'nothing to terminate'."""
    result=subprocess.run(["xcrun","simctl","terminate",device,"com.shin1ohno.yebisu"],text=True,capture_output=True)
    print("P11 terminate before reading the app's files: exit",result.returncode,(result.stderr or "").strip(),flush=True)
    if result.returncode not in (0,3):
        raise RuntimeError("could not terminate the app before reading its files (simctl exit %d)"%result.returncode)

def run(*args, **kwargs):
    print("P11 COMMAND",list(args),flush=True)
    result=subprocess.run(args,cwd=ROOT,text=True,capture_output=True,**kwargs)
    print(result.stdout,flush=True);print(result.stderr,flush=True);result.check_returncode();return result

def positive_control(spec,device,out,container):
    """The legacy outside-modal positive control: testP8NativeEditing, then validate_positive."""
    positive_started=time.time()
    try:
        positive=run("xcodebuild","test","-project",str(spec/"SmokeOracle.xcodeproj"),"-scheme","SmokeOracle","-destination","id="+device,"-derivedDataPath",str(ROOT/"target/ios-smoke/build"),"-resultBundlePath",str(out/"positive.xcresult"),"CODE_SIGNING_ALLOWED=NO","-only-testing:SmokeOracle/SmokeQA/testP8NativeEditing")
    finally:
        terminate(device)
    if "testP8NativeEditing]' passed" not in positive.stdout:raise RuntimeError("positive-control XCTest not executed")
    positive_records=[r for line in (container/"Documents/p0-smoke.jsonl").read_text().splitlines() if (r:=json.loads(line)).get("time",0)>=positive_started]
    positive_messages=[json.loads(line) for line in (container/"Documents/p4-peer.jsonl").read_text().splitlines()]
    (out/"positive-evidence.json").write_text(json.dumps({"records":positive_records,"messages":positive_messages},indent=2))
    validate_positive(positive_records,positive_messages)
    # Preserve the first receiving transcript before starting another app process.
    (container/"Documents/p4-peer.jsonl").unlink()

def main():
    import argparse
    parser=argparse.ArgumentParser(description=__doc__)
    # Runs only testP11Settings and its gates, skipping the positive control's
    # separate launch (validate_positive passes on the plain Simulator build, #233).
    parser.add_argument("--skip-positive",action="store_true",help="run only testP11Settings and its receiving gates")
    args=parser.parse_args()
    started=time.time()
    out=ROOT/"target/ios-p11"/time.strftime("%Y%m%d-%H%M%S");out.mkdir(parents=True)
    bundle=run("bash","scripts/ios/bundle.sh","--package","yebisu","--bin","yebisu")
    device=run("bash","-c",'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').stdout.split()[0]
    run("xcrun","simctl","install",device,bundle.stdout.strip().splitlines()[-1])
    container=Path(run("xcrun","simctl","get_app_container",device,"com.shin1ohno.yebisu","data").stdout.strip())
    (container/"Documents/p4-peer.jsonl").unlink(missing_ok=True)
    for file in container.rglob("p11-qa/yebisu-settings.json"):file.unlink()
    spec=ROOT/"scripts/ios/smoke-qa"
    run("xcodegen","generate","--spec",str(spec/"project.yml"),"--project",str(spec))
    # Exercise real selectors outside the modal before the separate P11 launch.
    terminate(device)  # an earlier check's app may still run
    if not args.skip_positive:
        positive_control(spec,device,out,container)
    else:
        print("P11 positive control skipped (--skip-positive)",flush=True)
        (container/"Documents/p4-peer.jsonl").unlink(missing_ok=True)
    started=time.time()
    try:
        tested=run("xcodebuild","test","-project",str(spec/"SmokeOracle.xcodeproj"),"-scheme","SmokeOracle","-destination","id="+device,"-derivedDataPath",str(ROOT/"target/ios-smoke/build"),"-resultBundlePath",str(out/"result.xcresult"),"CODE_SIGNING_ALLOWED=NO","-only-testing:SmokeOracle/SmokeQA/testP11Settings")
    finally:
        terminate(device)
    if "testP11Settings]' passed" not in tested.stdout:raise RuntimeError("P11 XCTest not executed")
    run("xcrun","xcresulttool","export","attachments","--path",str(out/"result.xcresult"),"--output-path",str(out/"attachments"))
    # Observation must append across relaunch for this probe.
    records=[record for line in (container/"Documents/p0-smoke.jsonl").read_text().splitlines() if (record:=json.loads(line)).get("time",0)>=started]
    messages=[json.loads(line) for line in (container/"Documents/p4-peer.jsonl").read_text().splitlines()]
    files=list(container.rglob("p11-qa/yebisu-settings.json"))
    if len(files)!=1:raise RuntimeError("missing unique isolated settings file")
    disk=json.loads(files[0].read_text())
    # Failed gates must retain raw receiving evidence, not just a traceback.
    (out/"evidence.json").write_text(json.dumps({"records":records,"messages":messages,"disk":disk},indent=2))
    (out/"observation.json").write_text(json.dumps(observations(records,messages,disk),indent=2))
    validate(records,messages,disk)
    print("P11 Simulator receiving gates passed; physical/Keychain gates remain owner-only.")
if __name__=="__main__":main()
