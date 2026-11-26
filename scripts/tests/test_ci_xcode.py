"""Every macOS job builds with Xcode 27.2 and iOS SDK 27.2 on the xcode-27 image.

Owner decision 2026-11-26: CI and TestFlight build with the latest iOS SDK.
The xcode-27 image defaults to Xcode 27.1, so each job selects 27.2 through
DEVELOPER_DIR and its first step fails unless the iphoneos SDK is 27.2.
"""
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CI = (ROOT / ".github/workflows/ci.yml").read_text()
TESTFLIGHT = (ROOT / ".github/workflows/testflight.yml").read_text()
DEVELOPER_DIR = "/Applications/Xcode_27.2.app/Contents/Developer"
GATE = "Require Xcode 27.2 and iOS SDK 27.2"
MACOS_JOBS = (
    (CI, "edit-menu"),
    (CI, "simulator-smoke"),
    (CI, "check"),
    (CI, "layout-smoke"),
    (TESTFLIGHT, "testflight"),
)


def job(workflow, name):
    match = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z][a-z-]*:\n|\Z)", workflow, re.S | re.M)
    if match is None:
        raise AssertionError(f"job {name} not found")
    return match.group(1)


def first_step(body):
    steps = body.split("\n    steps:\n", 1)[1]
    steps = re.sub(r"^\s*#.*\n", "", steps, flags=re.M)
    return re.split(r"^      - ", steps, flags=re.M)[1]


def gate_script(body):
    step = first_step(body)
    script = step.split("        run: |\n", 1)[1]
    return "".join(line[10:] + "\n" for line in script.splitlines() if line.startswith("          "))


class XcodeSelectionTests(unittest.TestCase):
    def test_every_runner_label_is_xcode_27_or_ubuntu(self):
        labels = re.findall(r"^    runs-on: (\S+)$", CI + TESTFLIGHT, re.M)
        self.assertEqual(len(labels), 7, "positive control: two ubuntu and five macOS jobs")
        self.assertEqual(sorted(set(labels)), ["ubuntu-latest", "xcode-27"])
        self.assertEqual(labels.count("xcode-27"), len(MACOS_JOBS))
        self.assertNotIn("macos-", CI + TESTFLIGHT)

    def test_each_macos_job_selects_xcode_27_2_and_gates_on_sdk_27_2_first(self):
        for workflow, name in MACOS_JOBS:
            with self.subTest(job=name):
                body = job(workflow, name)
                self.assertIn("    runs-on: xcode-27\n", body)
                env = re.search(r"^    env:\n((?:      .*\n)+)", body, re.M)
                self.assertIsNotNone(env, "job-level env")
                self.assertIn(f"      DEVELOPER_DIR: {DEVELOPER_DIR}\n", env.group(1))
                step = first_step(body)
                self.assertTrue(step.startswith(f"name: {GATE}\n"), step.split("\n", 1)[0])
                self.assertIn("xcrun --sdk iphoneos --show-sdk-version", step)
                self.assertNotIn("continue-on-error", step)

    def run_gate(self, sdk):
        script = gate_script(job(CI, "check"))
        with tempfile.TemporaryDirectory() as tmp:
            for tool, output in (("xcodebuild", "Xcode 27.2\nBuild version 27B52"), ("xcrun", sdk)):
                path = Path(tmp) / tool
                path.write_text(f"#!/bin/sh\nprintf '%s\\n' '{output}'\n")
                path.chmod(path.stat().st_mode | stat.S_IXUSR)
            env = {**os.environ, "PATH": f"{tmp}:/usr/bin:/bin"}
            return subprocess.run(["bash", "-e", "-c", script], env=env, capture_output=True, text=True)

    def test_the_gate_passes_only_sdk_27_2(self):
        passed = self.run_gate("27.2")
        self.assertEqual(passed.returncode, 0, passed.stdout + passed.stderr)
        for other in ("27.1", "27.0", "26.5", "27.2.1", ""):
            with self.subTest(sdk=other):
                failed = self.run_gate(other)
                self.assertEqual(failed.returncode, 1, failed.stdout)
                self.assertIn("::error::iphoneos SDK is", failed.stdout)

    def test_every_gate_runs_the_same_script(self):
        scripts = {name: gate_script(job(workflow, name)) for workflow, name in MACOS_JOBS}
        self.assertEqual(len(set(scripts.values())), 1, scripts)


if __name__ == "__main__":
    unittest.main()
