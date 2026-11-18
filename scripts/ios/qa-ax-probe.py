#!/usr/bin/env python3
"""S0 probe: run QAAxProbe/testS0Probe on fresh disposable iPad Simulators.

Builds the Simulator bundle once, then for each run creates a new iPad
Simulator, installs the app, runs the probe XCTest, keeps its log, the
xcresult and the app's own p0-smoke.jsonl, and deletes the Simulator.
Usage: qa-ax-probe.py OUTPUT_DIR [--runs N]
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

_spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
ci_smoke = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ci_smoke)

TEST = "SmokeOracle/QAAxProbe/testS0Probe"


def one_run(root, project, app_path, out, index, device_type, runtime):
    device = subprocess.check_output(
        ["xcrun", "simctl", "create", f"yebisu S0 probe {index}", device_type, runtime], text=True,
    ).strip()
    print(f"S0 run={index} simulator={device} runtime={runtime}", flush=True)
    log = out / f"run-{index}.log"
    try:
        subprocess.run(["xcrun", "simctl", "bootstatus", device, "-b"], check=True, capture_output=True)
        subprocess.run(["xcrun", "simctl", "install", device, app_path], check=True)
        result = out / f"run-{index}.xcresult"
        with log.open("w") as handle:
            tested = subprocess.run(
                ["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"),
                 "-scheme", "SmokeOracle", f"-only-testing:{TEST}", "-destination", "id=" + device,
                 "-derivedDataPath", str(root / "target/ios-smoke/build"),
                 "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO"],
                env={**os.environ, "TEST_RUNNER_QA_AX_PROBE": "1"},
                stdout=handle, stderr=subprocess.STDOUT, check=False,
            )
        container = subprocess.check_output(
            ["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], text=True,
        ).strip()
        jsonl = Path(container) / "Documents/p0-smoke.jsonl"
        if jsonl.exists():
            shutil.copyfile(jsonl, out / f"run-{index}-app.jsonl")
        passed = "testS0Probe]' passed" in log.read_text()
        print(f"S0 run={index} xcodebuild_exit={tested.returncode} passed={passed}", flush=True)
        return passed
    finally:
        shutdown = subprocess.run(["xcrun", "simctl", "shutdown", device], check=False, capture_output=True)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print(f"S0 run={index} cleanup simulator={device} shutdown_exit={shutdown.returncode} deleted", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    bundled = subprocess.run(["bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu"],
                             check=True, capture_output=True, text=True)
    app_path = bundled.stdout.strip().splitlines()[-1]
    print(f"S0 bundle={app_path}", flush=True)
    project = root / "scripts/ios/smoke-qa"
    subprocess.run(["xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project)],
                   check=True)
    listing = subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True)
    device_type, runtime = ci_smoke.select_template(json.loads(listing)["devices"])
    results = [one_run(root, project, app_path, out, index, device_type, runtime)
               for index in range(1, args.runs + 1)]
    print(f"S0 passed={sum(results)}/{len(results)}", flush=True)
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
