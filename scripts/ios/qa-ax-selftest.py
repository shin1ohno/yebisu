#!/usr/bin/env python3
"""S1 self-test of the QA accessibility layer on fresh disposable iPad Simulators.

Builds the Simulator bundle once, then for each run creates a new iPad Simulator, installs
the app, runs QAAxSelfTest (testQAAxSelfTest, testQAAxOverlappedPress (S3) and
testQAStateHoldsWhileMarksMove (issue #198)) and QAAxLogicTests
(it runs only when asked: TEST_RUNNER_QA_AX_SELFTEST),
keeps the test log and the xcresult, and deletes the Simulator. No existing job runs it.

  qa-ax-selftest.py OUTPUT_DIR [--runs N]
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

_spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
ci_smoke = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ci_smoke)

TESTS = ["SmokeOracle/QAAxSelfTest/testQAAxSelfTest", "SmokeOracle/QAAxSelfTest/testQAAxOverlappedPress",
         "SmokeOracle/QAAxSelfTest/testQAStateHoldsWhileMarksMove", "SmokeOracle/QAAxLogicTests"]
# A skipped test also passes: each self-test method must report its own passed line.
PASSED = ["testQAAxSelfTest]' passed", "testQAAxOverlappedPress]' passed", "testQAStateHoldsWhileMarksMove]' passed"]


def one_run(root, project, app_path, out, index, device_type, runtime):
    device = subprocess.check_output(
        ["xcrun", "simctl", "create", f"yebisu S1 self-test {index}", device_type, runtime], text=True,
    ).strip()
    print(f"S1 run={index} simulator={device} runtime={runtime}", flush=True)
    log = out / f"run-{index}.log"
    try:
        subprocess.run(["xcrun", "simctl", "bootstatus", device, "-b"], check=True, capture_output=True)
        subprocess.run(["xcrun", "simctl", "install", device, app_path], check=True)
        with log.open("w") as handle:
            tested = subprocess.run(
                ["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"),
                 "-scheme", "SmokeOracle", *[f"-only-testing:{test}" for test in TESTS], "-destination", "id=" + device,
                 "-derivedDataPath", str(root / "target/ios-smoke/build"),
                 "-resultBundlePath", str(out / f"run-{index}.xcresult"), "CODE_SIGNING_ALLOWED=NO"],
                env={**os.environ, "TEST_RUNNER_QA_AX_SELFTEST": "1"},
                stdout=handle, stderr=subprocess.STDOUT, check=False,
            )
        text = log.read_text()
        # A skipped test (no TEST_RUNNER_QA_AX_SELFTEST) also exits 0: require the passed line.
        passed = all(line in text for line in PASSED) and tested.returncode == 0
        print(f"S1 run={index} xcodebuild_exit={tested.returncode} passed={passed}", flush=True)
        for line in text.splitlines():
            if line.startswith("QAAX "):
                print(f"S1 run={index} {line}", flush=True)
        return passed
    finally:
        shutdown = subprocess.run(["xcrun", "simctl", "shutdown", device], check=False, capture_output=True)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print(f"S1 run={index} cleanup simulator={device} shutdown_exit={shutdown.returncode} deleted", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("output", type=Path)
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.run(["git", "diff", "--quiet", "HEAD"], check=False).returncode != 0
    print(f"S1 head={head} dirty={dirty}", flush=True)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    bundled = subprocess.run(["bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu"],
                             check=True, capture_output=True, text=True)
    app_path = bundled.stdout.strip().splitlines()[-1]
    print(f"S1 bundle={app_path}", flush=True)
    project = root / "scripts/ios/smoke-qa"
    subprocess.run(["xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project)],
                   check=True)
    listing = subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True)
    device_type, runtime = ci_smoke.select_template(json.loads(listing)["devices"])
    results = [one_run(root, project, app_path, out, index, device_type, runtime)
               for index in range(1, args.runs + 1)]
    print(f"S1 head={head} passed={sum(results)}/{len(results)}", flush=True)
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
