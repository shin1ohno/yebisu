#!/usr/bin/env python3
"""Run P2 UI on Simulator with an injected key; never read keys from logs."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def run(*command, **kwargs):
    print("P2 COMMAND", list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


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
    if not any(v.get("status") == "Copied restricted public key." for v in records):
        raise RuntimeError("copy action did not complete in the receiving application")
    return "restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa"

def main():
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    start = time.time()
    destination = root / "target/ios-p2" / time.strftime("%Y%m%d-%H%M%S")
    destination.mkdir(parents=True)
    bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu", capture_output=True)
    print(bundle.stderr, flush=True)
    app_path = bundle.stdout.strip().splitlines()[-1]
    environment = dict(os.environ, SIMCTL_CHILD_YEBISU_P2_OBSERVE="1", SIMCTL_CHILD_YEBISU_P11_KEYS_QA="1")
    run("bash", "scripts/ios/sim.sh", "--wait", "3", "--out", str(destination / "launch"), app_path, env=environment)
    device = subprocess.check_output(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
    preflight_container=Path(subprocess.check_output(["xcrun","simctl","get_app_container",device,"com.shin1ohno.yebisu","data"],text=True).strip())
    preflight=[r for line in (preflight_container/"Documents/p2-setup.jsonl").read_text().splitlines() if (r:=json.loads(line)).get("time",0)>=start]
    validate_preflight(preflight)
    (destination/"provider-preflight.json").write_text(json.dumps(preflight,indent=2))
    spec = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(spec / "project.yml"), "--project", str(spec))
    run("xcodebuild", "test", "-project", str(spec / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
        "-destination", "id=" + device, "-derivedDataPath", str(root / "target/ios-smoke/build"),
        "-resultBundlePath", str(destination / "result.xcresult"), "CODE_SIGNING_ALLOWED=NO",
        "-only-testing:SmokeOracle/SmokeQA/testP2KeysAndRestart")
    attachments = destination / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(destination / "result.xcresult"), "--output-path", str(attachments))
    screenshots = [a for test in json.loads((attachments / "manifest.json").read_text()) for a in test["attachments"]
                   if a["suggestedHumanReadableName"].startswith("p2-restarted")]
    if len(screenshots) != 1:
        raise RuntimeError("missing P2 foreground screenshot before XCTest teardown")
    shutil.copyfile(attachments / screenshots[0]["exportedFileName"], destination / "screenshot.png")
    run("python3", "scripts/ios/screenshot-color.py", str(destination / "screenshot.png"), "2e3440", "--min", "0.4")
    container = subprocess.check_output(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True).strip()
    text = (Path(container) / "Documents/p2-setup.jsonl").read_text()
    records = [v for line in text.splitlines() if (v := json.loads(line)).get("time", 0) >= start]
    expected_line = validate_records(records)
    clipboard = subprocess.check_output(["xcrun", "simctl", "pbpaste", device], text=True)
    if clipboard != expected_line:
        raise RuntimeError("receiving Simulator pasteboard differs from the formatted public key")
    print("P2 receiving Simulator pasteboard exactly matches the restricted public key.")
    # Negative controls use the actual app's receipt shape, not invented output.
    for corrupted, expected in [([], "missing independent process launch receipts"),
                                ([v for v in records if v.get("status") != "Copied restricted public key."],
                                 "copy action did not complete in the receiving application")]:
        try:
            validate_records(corrupted)
        except RuntimeError as error:
            if str(error) != expected:
                raise
            print("P2 negative control refused:", expected)
        else:
            raise RuntimeError("negative control unexpectedly passed")
    (destination / "app.jsonl").write_text("".join(json.dumps(v) + "\n" for v in records))
    print("P2 PASS: source-defined controls found by OCR; injected key displayed and copied after process restart.")
    print("Injected provider only; Keychain persistence, Secure Enclave, real authentication and Local Network remain owner checks.")
    (root / "target/ios-p2/latest.json").write_text(json.dumps({"directory": str(destination)}) + "\n")
    print("P2 RECEIVING EVIDENCE", destination, flush=True)


if __name__ == "__main__":
    main()
