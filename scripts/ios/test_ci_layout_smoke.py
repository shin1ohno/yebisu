"""The main-only layout-smoke job and the driver it runs."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import re
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


menu = load("test_ci_sh1_152", "test_ci_sh1_152.py")  # decisions() runs the real filter step in a throwaway repository
driver = load("ci_layout_smoke", "ci-layout-smoke.py")

CHECKS = ["p2-check.py", "p3-check.py", "p10-check.py"]


class JobTests(unittest.TestCase):
    def job(self):
        match = re.search(r"^  layout-smoke:\n(.*?)(?=^  [a-z][a-z-]*:\n|\Z)", WORKFLOW, re.S | re.M)
        self.assertIsNotNone(match)
        return match.group(1)

    def test_one_job_runs_the_driver_within_sixty_minutes(self):
        job = self.job()
        self.assertIn("runs-on: macos-26\n", job)
        self.assertIn("timeout-minutes: 60\n", job)
        self.assertIn(
            "if: github.event_name != 'pull_request' && github.ref == 'refs/heads/main'"
            " && needs.changes.outputs.layout-smoke == 'true'\n", job
        )
        self.assertIn("run: just guarded python3 scripts/ios/ci-layout-smoke.py\n", job)
        self.assertNotIn("matrix", job)

    def test_no_retry_or_continue_on_error(self):
        job = self.job()
        self.assertNotIn("retry", job.lower().replace("receiving-system", ""))
        self.assertNotIn("continue-on-error", job)

    def test_built_like_the_edit_menu_job(self):
        edit_menu = re.search(r"^  edit-menu:\n(.*?)^  simulator-smoke:\n", WORKFLOW, re.S | re.M).group(1)
        for step in ("python3 scripts/guarded.py rustup show active-toolchain", "brew install just xcodegen", "just guarded cargo fetch --locked"):
            with self.subTest(step=step):
                self.assertIn(step, edit_menu)
                self.assertIn(step, self.job())

    def test_evidence_of_every_check_and_the_durations_are_uploaded_even_after_a_failure(self):
        job = self.job()
        self.assertIn("if: always()\n", job)
        self.assertIn("name: layout-smoke-evidence\n", job)
        for path in ("target/ios-layout-smoke/", "target/ios-p2/", "target/ios-p3/", "target/ios-p10/"):
            self.assertIn(path, job)

    def test_the_job_is_selected_by_the_changes_job(self):
        self.assertIn("layout-smoke: ${{ steps.filter.outputs.layout-smoke }}\n", WORKFLOW)

    def test_the_driver_runs_the_three_checks_in_order(self):
        self.assertEqual(driver.CHECKS, CHECKS)
        text = (HERE / "ci-layout-smoke.py").read_text()
        line = re.search(r"^CHECKS = \[(.*)\]$", text, re.M).group(1)
        self.assertEqual(re.findall(r'"([a-z0-9_-]+-check\.py)"', line), CHECKS)
        for name in CHECKS:
            self.assertTrue((HERE / name).is_file(), name)

    def test_known_red_is_empty_since_p3_passed_on_hosted(self):
        # Integration 2: p3 passed every stage in run 44214913068, so no check is known red.
        self.assertEqual(driver.KNOWN_RED, {})
        for name in driver.KNOWN_RED:
            self.assertIn(name, CHECKS)

    def results(self, p2=0, p3=1, p10=0):
        return [{"check": "p2-check.py", "exit": p2}, {"check": "p3-check.py", "exit": p3}, {"check": "p10-check.py", "exit": p10}]

    # The known-red mechanism, exercised with p3 listed as it was in Integration 1.
    P3_RED = {"p3-check.py": "landscape rotation drops the private-peer connection (aed154e)"}

    @mock.patch.dict(driver.KNOWN_RED, P3_RED)
    def test_a_known_red_failure_is_logged_and_does_not_fail_the_job(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(driver.verdict(self.results()), [])
        self.assertIn("layout-smoke KNOWN RED: p3-check.py", out.getvalue())

    def test_negative_other_failures_still_fail_the_job(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(driver.verdict(self.results(p3=1)), ["p3-check.py"])
            self.assertEqual(driver.verdict(self.results(p2=1, p3=0)), ["p2-check.py"])
            self.assertEqual(driver.verdict(self.results(p3=0, p10=65)), ["p10-check.py"])
            self.assertEqual(driver.verdict(self.results(p3=0)[:2]), ["a check did not run"])

    @mock.patch.dict(driver.KNOWN_RED, P3_RED)
    def test_negative_a_known_red_that_passes_fails_the_job_until_removed(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(driver.verdict(self.results(p3=0)), ["p3-check.py"])
        self.assertIn("remove it from KNOWN_RED", out.getvalue())

    def test_every_check_the_driver_runs_takes_its_device_from_the_environment(self):
        for name in CHECKS:
            with self.subTest(check=name):
                self.assertIn("HERDR_IOS_SIMULATOR", (HERE / name).read_text())


class SelectionTests(unittest.TestCase):
    def test_pull_request_never_selects_layout_smoke(self):
        for path in (".github/workflows/ci.yml", "scripts/ios/p3-check.py",
                     "crates/yebisu/src/layout.rs", "crates/yebisu/src/shell/layout_tests.rs"):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as tmp:
                self.assertFalse(menu.decisions("pull_request", [path], tmp)["layout-smoke"])

    def test_main_push_schedule_and_manual_run_select_layout_smoke(self):
        for event in ("push", "schedule", "workflow_dispatch"):
            with self.subTest(event=event), tempfile.TemporaryDirectory() as tmp:
                self.assertTrue(menu.decisions(event, ["docs/ci-simulator.md"], tmp)["layout-smoke"])


class DriverTests(unittest.TestCase):
    """The driver with its subprocess calls replaced: order, durations, cleanup and the exit status."""

    def run_main(self, exits, boot_fails=False):
        calls = []
        evidence = tempfile.TemporaryDirectory()
        self.addCleanup(evidence.cleanup)
        queue = list(exits)

        def fake_run(command, **kwargs):
            calls.append(command)
            if command[:2] == ["bash", "-c"]:
                if boot_fails:
                    raise driver.subprocess.CalledProcessError(1, command)
                return mock.Mock(returncode=0)
            if command[:1] == [driver.sys.executable]:
                return mock.Mock(returncode=queue.pop(0))
            return mock.Mock(returncode=0)

        def fake_output(command, **kwargs):
            return json.dumps({"devices": {}}) if "list" in command else "FAKE-UDID\n"

        with mock.patch.object(driver.subprocess, "run", fake_run), mock.patch.object(driver.subprocess, "check_output", fake_output), \
                mock.patch.object(driver.smoke, "select_template", return_value=("iPad", "iOS-27-0")), \
                mock.patch.object(driver, "EVIDENCE", Path(evidence.name)):
            with contextlib.redirect_stdout(io.StringIO()):
                try:
                    driver.main()
                    code = 0
                except SystemExit as stop:
                    code = stop.code
                except driver.subprocess.CalledProcessError:
                    code = "error"
        durations = json.loads((Path(evidence.name) / "durations.json").read_text())
        return code, calls, durations

    def scripts_run(self, calls):
        return [Path(c[1]).name for c in calls if c[:1] == [driver.sys.executable]]

    @mock.patch.dict(driver.KNOWN_RED, {"p3-check.py": "known red for this test"})
    def test_all_three_checks_run_in_order_on_one_simulator_that_is_deleted(self):
        # With p3 known red, its failure alone leaves the job green.
        code, calls, durations = self.run_main([0, 1, 0])
        self.assertEqual(code, 0)
        self.assertEqual(self.scripts_run(calls), CHECKS)
        self.assertEqual(sum(1 for c in calls if c[:3] == ["xcrun", "simctl", "delete"]), 1)
        self.assertEqual([c["check"] for c in durations["checks"]], CHECKS)
        self.assertEqual([c["exit"] for c in durations["checks"]], [0, 1, 0])
        self.assertIn("total_seconds", durations)

    def test_a_failing_check_does_not_stop_the_others_but_fails_the_job(self):
        code, calls, durations = self.run_main([1, 1, 0])
        self.assertEqual(code, 1)
        self.assertEqual(self.scripts_run(calls), CHECKS)
        self.assertEqual([c["exit"] for c in durations["checks"]], [1, 1, 0])
        self.assertEqual(sum(1 for c in calls if c[:3] == ["xcrun", "simctl", "delete"]), 1)

    @mock.patch.dict(driver.KNOWN_RED, {"p3-check.py": "known red for this test"})
    def test_a_known_red_that_passes_fails_the_job(self):
        code, _, _ = self.run_main([0, 0, 0])
        self.assertEqual(code, 1)

    def test_a_boot_failure_still_deletes_the_simulator(self):
        code, calls, durations = self.run_main([], boot_fails=True)
        self.assertEqual(code, "error")
        self.assertEqual(self.scripts_run(calls), [])
        self.assertEqual(sum(1 for c in calls if c[:3] == ["xcrun", "simctl", "delete"]), 1)
        self.assertEqual(durations["checks"], [])


if __name__ == "__main__":
    unittest.main()
