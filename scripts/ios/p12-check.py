#!/usr/bin/env python3
"""Run only under an exclusive Mac lease. Real OS menu + consumer + peer evidence."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("p12_oracle", Path(__file__).with_name("p12-oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("Exclusive Mac lease required; no Linux Rust build")
    directory = args.output.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    steps = []

    def run(label, command, guarded=False, check=True):
        if guarded:
            command = ["python3", "scripts/guarded.py", *command]
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        (directory / f"{label}.stdout").write_text(result.stdout)
        (directory / f"{label}.stderr").write_text(result.stderr)
        steps.append({"label": label, "command": command, "exit": result.returncode})
        (directory / "steps.json").write_text(json.dumps(steps, indent=2) + "\n")
        print(f"P12 {label}: exit {result.returncode}", flush=True)
        if check:
            result.check_returncode()
        return result

    head = run("head", ["git", "rev-parse", "HEAD"]).stdout.strip()
    run("xcode-version", ["xcodebuild", "-version"])
    run("devices", ["xcrun", "simctl", "list", "--json", "devices", "available"])
    bundle = run("bundle", ["bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu"], guarded=True)
    device = run("boot", ["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"']).stdout.strip().splitlines()[-1].split()[0]
    run("install", ["xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1]])
    container = Path(run("container", ["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"]).stdout.strip())
    documents = container / "Documents"
    documents.mkdir(exist_ok=True)
    # Preserve previous QA log before starting a new, unambiguous peer stream.
    peer = documents / "p4-peer.jsonl"
    if peer.exists():
        shutil.copyfile(peer, directory / "prior-peer.jsonl")
    peer.write_text("")
    project = directory / "project"
    shutil.copytree(ROOT / "scripts/ios/p12-qa", project)
    run("xcodegen", ["xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project)])
    result_path = directory / "result.xcresult"
    derived = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "ios-p12-build"
    tested = run("xctest", ["xcodebuild", "test", "-project", str(project / "P12Oracle.xcodeproj"),
        "-scheme", "P12Oracle", "-destination", "id=" + device, "-derivedDataPath", str(derived),
        "-resultBundlePath", str(result_path), "CODE_SIGNING_ALLOWED=NO",
        "-only-testing:P12Oracle/P12QA/testInstalledTerminalMenu"], guarded=True, check=False)
    # Collect receiving evidence even on XCTest failure; never replace it with a model.
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline and "p12-fence" not in peer.read_text():
        time.sleep(0.1)
    for source, target in [("p0-smoke.jsonl", "app.jsonl"), ("p4-peer.jsonl", "peer.jsonl")]:
        if (documents / source).exists():
            shutil.copyfile(documents / source, directory / target)
    run("terminate", ["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], check=False)
    export = directory / "attachments"
    run("attachments", ["xcrun", "xcresulttool", "export", "attachments", "--path", str(result_path), "--output-path", str(export)])
    screenshots, ui = set(), None
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            name = attachment["suggestedHumanReadableName"]
            data = (export / attachment["exportedFileName"]).read_bytes()
            if name.startswith("p12-ui-events"):
                ui = json.loads(data)
            for expected in ["p12-finished", *[f"p12-{stage}-{case}" for stage in ["menu", "before", "after"] for case in oracle.CASES]]:
                if name.startswith(expected) and (data.startswith(b"\x89PNG\r\n\x1a\n") or data.startswith(b"\xff\xd8")):
                    screenshots.add(expected)
    tested.check_returncode()
    oracle.require(ui is not None, "actual XCTest UI event attachment missing")
    records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
    verdict = oracle.validate(records, messages, ui, screenshots)
    verdict.update(head=head, device=device, screenshot_count=len(screenshots))
    (directory / "verdict.json").write_text(json.dumps(verdict, indent=2) + "\n")
    print(json.dumps(verdict), flush=True)


if __name__ == "__main__":
    main()
