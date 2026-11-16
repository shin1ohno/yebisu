"""ci-sh1-152.py runs one named check per Simulator; ci.yml matrix and path filter agree with it."""
import contextlib
import importlib.util
import io
from pathlib import Path
import re
import subprocess
import sys
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
WORKFLOW = (HERE.parents[1] / ".github/workflows/ci.yml").read_text()

spec = importlib.util.spec_from_file_location("ci_sh1_152", HERE / "ci-sh1-152.py")
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)

DEVICES = '{"devices": {"com.apple.CoreSimulator.SimRuntime.iOS-26-5": [{"name": "iPad Pro 13-inch (M4)", "isAvailable": true, "deviceTypeIdentifier": "type"}]}}'
UDID = "11111111-2222-3333-4444-555555555555"


class Recorder:
    """Stands in for subprocess.check_output / subprocess.run and records the calls."""

    def __init__(self, fail_script=None):
        self.runs, self.fail_script = [], fail_script

    def check_output(self, command, **kwargs):
        return DEVICES if "list" in command else UDID + "\n"

    def run(self, command, **kwargs):
        self.runs.append((command, kwargs))
        if self.fail_script and command[-1].endswith(self.fail_script):
            raise subprocess.CalledProcessError(1, command)
        return subprocess.CompletedProcess(command, 0)

    def checks(self):
        return [Path(c[-1]).name for c, _ in self.runs if c[0] == sys.executable]

    def simctl(self):
        return [c[2:4] for c, _ in self.runs if c[:2] == ["xcrun", "simctl"]]


def run_main(argv, fail_script=None):
    recorder = Recorder(fail_script)
    with patch.object(ci.subprocess, "check_output", recorder.check_output), patch.object(ci.subprocess, "run", recorder.run), \
            contextlib.redirect_stdout(io.StringIO()):
        try:
            ci.main(argv)
            error = None
        except subprocess.CalledProcessError as raised:
            error = raised
    return recorder, error


class ArgumentTests(unittest.TestCase):
    def test_each_name_selects_exactly_its_script(self):
        self.assertEqual({name: ci.scripts_for(name) for name in ci.CHECKS},
                         {"sh1-152": ["sh1-152-check.py"], "sh1-153": ["sh1-153-check.py"],
                          "p8-native": ["p8-native-check.py"], "selection-settings": ["selection-settings-check.py"]})

    def test_all_runs_the_four_scripts_in_the_old_order(self):
        self.assertEqual(ci.scripts_for("all"),
                         ["sh1-152-check.py", "sh1-153-check.py", "p8-native-check.py", "selection-settings-check.py"])

    def test_every_script_exists(self):
        for script in ci.CHECKS.values():
            self.assertTrue((HERE / script).is_file(), script)

    def test_missing_or_unknown_or_extra_argument_is_rejected(self):
        for argv in ([], ["sh1-154"], ["sh1-152", "sh1-153"], ["--all"]):
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as raised:
                ci.parse_args(argv)
            self.assertEqual(raised.exception.code, 2)


class MainTests(unittest.TestCase):
    def test_one_check_runs_only_that_script_on_one_simulator_and_cleans_up(self):
        recorder, error = run_main(["p8-native"])
        self.assertIsNone(error)
        self.assertEqual(recorder.checks(), ["p8-native-check.py"])
        env = next(kw["env"] for c, kw in recorder.runs if c[0] == sys.executable)
        self.assertEqual(env["HERDR_IOS_SIMULATOR"], UDID)
        self.assertEqual(recorder.simctl()[-2:], [["shutdown", UDID], ["delete", UDID]])

    def test_all_runs_four_scripts_on_one_simulator(self):
        recorder, error = run_main(["all"])
        self.assertIsNone(error)
        self.assertEqual(recorder.checks(), list(ci.CHECKS.values()))
        self.assertEqual(recorder.simctl().count(["delete", UDID]), 1)

    def test_failing_check_still_shuts_down_and_deletes_and_stops_the_run(self):
        recorder, error = run_main(["all"], fail_script="sh1-153-check.py")
        self.assertEqual(error.cmd[-1].endswith("sh1-153-check.py"), True)
        self.assertEqual(recorder.checks(), ["sh1-152-check.py", "sh1-153-check.py"])
        self.assertEqual(recorder.simctl()[-2:], [["shutdown", UDID], ["delete", UDID]])

    def test_failed_shutdown_does_not_prevent_deletion(self):
        recorder = Recorder()
        original = recorder.run

        def run(command, **kwargs):
            result = original(command, **kwargs)
            return subprocess.CompletedProcess(command, 1 if command[2:3] == ["shutdown"] else 0)

        with patch.object(ci.subprocess, "check_output", recorder.check_output), patch.object(ci.subprocess, "run", run), \
                contextlib.redirect_stdout(io.StringIO()):
            ci.main(["sh1-152"])
        self.assertEqual(recorder.simctl()[-2:], [["shutdown", UDID], ["delete", UDID]])


class WorkflowTests(unittest.TestCase):
    def job(self):
        match = re.search(r"^  edit-menu:\n(.*?)^  simulator-smoke:\n", WORKFLOW, re.S | re.M)
        self.assertIsNotNone(match)
        return match.group(1)

    def test_matrix_names_are_the_checks_and_each_has_its_own_evidence(self):
        job = self.job()
        names = re.findall(r"- check: (\S+)\n\s+evidence: (\S+)\n", job)
        self.assertEqual([n for n, _ in names], list(ci.CHECKS))
        self.assertEqual([e for _, e in names], ["target/ios-sh1-152/", "target/ios-sh1-153/", "target/ios-p8-native/", "target/ios-selection-settings/"])
        self.assertIn("scripts/ios/ci-sh1-152.py ${{ matrix.check }}", job)
        self.assertIn("name: edit-menu-evidence-${{ matrix.check }}", job)
        self.assertIn("fail-fast: false", job)

    def test_timeout_and_guard_are_not_loosened(self):
        job = self.job()
        self.assertIn("timeout-minutes: 90\n", job)
        self.assertIn("run: just guarded python3 scripts/ios/ci-sh1-152.py", job)
        self.assertNotIn("retry", job.lower())
        self.assertNotIn("continue-on-error", job)

    def test_each_check_script_reads_its_own_evidence_directory(self):
        for name, evidence in [("sh1-152-check.py", "ios-sh1-152"), ("sh1-153-check.py", "ios-sh1-153"),
                               ("p8-native-check.py", "ios-p8-native"), ("selection-settings-check.py", "ios-selection-settings")]:
            self.assertIn(f'ROOT / "target/{evidence}"', (HERE / name).read_text(), name)


class PathFilterTests(unittest.TestCase):
    """Applies the pattern ci.yml's changes job feeds to `grep -Eq` to sample changed-file lists."""

    @classmethod
    def setUpClass(cls):
        build = re.search(r"^          build='(.*)'$", WORKFLOW, re.M).group(1)
        menu = re.search(r"""^          menu="\$build\|"'(.*)'$""", WORKFLOW, re.M).group(1)
        cls.pattern = re.compile(f"{build}|{menu}")

    def runs(self, *paths):
        return any(self.pattern.search(path) for path in paths)

    def test_docs_only_and_p12_only_pull_requests_skip(self):
        self.assertFalse(self.runs("docs/ci-simulator.md", "README.md", "TODO.md"))
        self.assertFalse(self.runs("scripts/ios/p12-qa/Tests/P12QA.swift", "scripts/ios/p12-qa/project.yml", "scripts/ios/p12-ci.py",
                                   "scripts/ios/p12-check.py", "scripts/ios/p12-oracle.py", "scripts/ios/test_p12_oracle.py",
                                   "scripts/ios/p12-mutations.py", "docs/p12-keyless-menus.md"))
        self.assertFalse(self.runs("scripts/ios/sim.sh", "scripts/ios/smoke-check.py", "scripts/ios/p9-check.py", "scripts/ios/test_ci_sh1_152.py",
                                   "scripts/ios/test_sh1_152_check.py", "scripts/release/check-commit-messages.py", ".github/workflows/testflight.yml",
                                   "scripts/ios/fixtures/settings-writer-default.json"))

    def test_every_file_the_checks_exercise_runs_the_jobs(self):
        for path in (
            "crates/yebisu/src/edit_menu.rs", "crates/yebisu/Cargo.toml",
            "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", "assets/fonts/SauceCodePro/LICENSE.txt",
            "justfile", "scripts/guarded.py", ".github/workflows/ci.yml",
            "scripts/ios/bundle.sh", "scripts/ios/Info.plist", "scripts/ios/lib.sh", "scripts/ios/ci-smoke.py", "scripts/ios/ci-sh1-152.py",
            "scripts/ios/sh1-152-check.py", "scripts/ios/sh1-153-check.py", "scripts/ios/p8-native-check.py", "scripts/ios/selection-settings-check.py",
            "scripts/ios/sh1_152_oracle.py", "scripts/ios/sh1_152_scenes.py", "scripts/ios/sh1_152_transitions.py",
            "scripts/ios/smoke-qa/Tests/EditMenuQA.swift", "scripts/ios/smoke-qa/Tests/SmokeQA.swift", "scripts/ios/smoke-qa/project.yml",
        ):
            with self.subTest(path=path):
                self.assertTrue(self.runs(path))

    def test_a_p12_pull_request_that_edits_the_app_still_runs(self):
        self.assertTrue(self.runs("scripts/ios/p12-qa/Tests/P12QA.swift", "crates/yebisu/src/native_menus_qa.rs"))


if __name__ == "__main__":
    unittest.main()
