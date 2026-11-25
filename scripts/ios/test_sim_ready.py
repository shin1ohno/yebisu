"""sim_ready.wait_for_destination (issue #142 stage 4, class D) and its call sites."""
import contextlib
import importlib.util
import io
from pathlib import Path
import re
import subprocess
import types
import unittest

HERE = Path(__file__).parent
spec = importlib.util.spec_from_file_location("sim_ready", HERE / "sim_ready.py")
sim_ready = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sim_ready)
budget_spec = importlib.util.spec_from_file_location("resend_budget", HERE / "resend_budget.py")
budget = importlib.util.module_from_spec(budget_spec)
budget_spec.loader.exec_module(budget)

UDID = "99ECD66A-0000-4000-8000-000000000001"
LISTING = "\n".join([
    "Available destinations for the \"SmokeOracle\" scheme:",
    "\t{ platform:iOS Simulator, arch:arm64, id:00000000-1111-2222-3333-444444444444, OS:26.5, name:iPad Air }",
    "\t{ platform:iOS Simulator, arch:arm64, id:%s, OS:26.5, name:yebisu CI smoke }" % UDID,
])


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def clock(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


def lister(listed_from):
    """A fake subprocess.run whose listing names the device from call `listed_from` on (never when None)."""
    calls = []

    def run(command, **kwargs):
        calls.append(command)
        listed = listed_from is not None and len(calls) >= listed_from
        return types.SimpleNamespace(stdout=LISTING if listed else LISTING.split("\n")[0], stderr="", returncode=0)
    return run, calls


class WaitTests(unittest.TestCase):
    def wait(self, listed_from, **kwargs):
        fake = FakeClock()
        run, calls = lister(listed_from)
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            waited = sim_ready.wait_for_destination(Path("spec/SmokeOracle.xcodeproj"), "SmokeOracle", UDID, run=run,
                                                    clock=fake.clock, sleep=fake.sleep, **kwargs)
        return waited, calls, printed.getvalue()

    def test_a_listed_device_returns_at_once(self):
        waited, calls, printed = self.wait(1)
        self.assertEqual(waited, 0)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0], ["xcodebuild", "-showdestinations", "-project", "spec/SmokeOracle.xcodeproj", "-scheme", "SmokeOracle"])
        self.assertIn(f"SIM READY {UDID} after 0.0 s (1 listings)", printed)

    def test_a_late_device_is_waited_for_and_the_wait_is_logged(self):
        waited, calls, printed = self.wait(4)
        self.assertEqual(waited, 9.0)
        self.assertEqual(len(calls), 4)
        self.assertIn(f"SIM READY {UDID} after 9.0 s (4 listings)", printed)

    def test_a_device_that_never_appears_is_reported_not_failed(self):
        """Owner decision 2026-11-25 (issue #142 PR-4): the listing is a report; xcodebuild itself decides."""
        waited, calls, printed = self.wait(None)
        self.assertIsNone(waited)
        self.assertEqual(len(calls), 21)
        self.assertRegex(printed.strip(), rf"^SIM NOT LISTED {UDID} after 60\.0 s \(21 listings\); "
                                          r"last listing exit 0: Available destinations for the \"SmokeOracle\" scheme:$")

    def test_another_device_does_not_count(self):
        self.assertFalse(sim_ready.listed(LISTING.replace(UDID, "other"), UDID))
        self.assertTrue(sim_ready.listed(LISTING, UDID))
        self.assertEqual(sim_ready.LIMIT, 60.0)


MISSING = ("xcodebuild: error: Unable to find a device matching the provided destination specifier:\n"
           "\t\t{ id:%s }" % UDID)
COMMAND = ["python3", "scripts/guarded.py", "xcodebuild", "test", "-project", "spec/SmokeOracle.xcodeproj", "-scheme", "SmokeOracle",
           "-destination", "id=" + UDID, "-only-testing:SmokeOracle/QAAxLogicTests"]


class RunTestTests(unittest.TestCase):
    """sim_ready.run_test: one resend, logged for the job's budget, only when no device was found and no test ran."""

    def run_with(self, *results):
        outputs = list(results)
        calls, waits = [], []

        def runner():
            calls.append(1)
            code, stdout, stderr = outputs.pop(0)
            return subprocess.CompletedProcess(COMMAND, code, stdout, stderr)
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            result = sim_ready.run_test(COMMAND, runner, wait=lambda *args: waits.append(args))
        return result, len(calls), waits, printed.getvalue()

    def test_a_found_device_runs_once(self):
        result, calls, waits, _ = self.run_with((0, "Test Suite 'All tests' passed", ""))
        self.assertEqual((result.returncode, calls, waits), (0, 1, []))
        self.assertNotIn("QA-RESEND", result.stdout)

    def test_a_missing_device_is_sent_once_more_and_logged(self):
        result, calls, waits, printed = self.run_with((70, "", MISSING), (0, "Test Suite 'All tests' passed", ""))
        self.assertEqual((result.returncode, calls), (0, 2))
        self.assertEqual(waits, [("spec/SmokeOracle.xcodeproj", "SmokeOracle", UDID)])
        resends = budget.parse(result.stdout)
        self.assertEqual([(r["kind"], r["id"]) for r in resends], [("destination-retry", UDID)])
        self.assertIn("QA-RESEND", printed)

    def test_a_device_missing_twice_fails_after_one_resend(self):
        result, calls, _, _ = self.run_with((70, "", MISSING), (70, "", MISSING))
        self.assertEqual((result.returncode, calls), (70, 2))
        self.assertEqual(len(budget.parse(result.stdout)), 1)

    def test_a_run_that_started_tests_is_never_sent_again(self):
        started = "Test Suite 'Selected tests' started\nTest Case '-[SmokeOracle.QAAxLogicTests testX]' started\n" + MISSING
        result, calls, waits, _ = self.run_with((65, started, ""))
        self.assertEqual((result.returncode, calls, waits), (65, 1, []))
        self.assertFalse(sim_ready.destination_missing(started))
        self.assertTrue(sim_ready.destination_missing(MISSING))
        self.assertEqual(sim_ready.destination(COMMAND), UDID)


# Every hosted `xcodebuild test` (smoke-check, p9 and p12 in simulator-smoke; the six edit-menu scripts;
# layout-smoke's p2, p3 and p10, plan Q1) and the project and scheme it tests.
SITES = {"smoke-check.py": "SmokeOracle", "p9-check.py": "SmokeOracle", "p12-check.py": "P12Oracle",
         "sh1-152-check.py": "SmokeOracle", "sh1_152_targets.py": "SmokeOracle", "sh1-153-check.py": "SmokeOracle",
         "p8-native-check.py": "SmokeOracle", "selection-settings-check.py": "SmokeOracle", "sh1-156-check.py": "SmokeOracle",
         "p2-check.py": "SmokeOracle", "p3-check.py": "SmokeOracle", "p10-check.py": "SmokeOracle"}


class CallSiteTests(unittest.TestCase):
    def test_every_hosted_xcodebuild_test_waits_for_its_destination_first(self):
        for name, scheme in SITES.items():
            with self.subTest(script=name):
                text = (HERE / name).read_text()
                call = text.index("sim_ready.wait_for_destination(")
                self.assertIn(f'"{scheme}.xcodeproj", "{scheme}", device)', text[call:call + 160])
                self.assertEqual(text.count("sim_ready.wait_for_destination("), 1)
                test_runs = [m.start() for m in re.finditer(r"""["']xcodebuild["'],\s*["']test["']""", text)]
                runs = [m.start() for m in re.finditer(r"xcodebuild_test_command\(", text)]
                used = [at for at in test_runs + runs if at > text.index("def main(")] if "def main(" in text else test_runs
                self.assertTrue(used, "no xcodebuild test found")
                self.assertTrue(all(call < at for at in used if at > call - 1) and call < max(used),
                                "the wait must come before the xcodebuild test run")
                self.assertLess(text.index("xcodegen"), call)
                code = "\n".join(l for l in text.splitlines() if not l.strip().startswith("#"))
                self.assertIn("sim_ready.run_test(", code, "the xcodebuild test run goes through run_test")

    def test_the_drivers_run_only_these_hosted_checks(self):
        """A new hosted check must be added here (and get the wait)."""
        smoke = (HERE / "ci-smoke.py").read_text()
        self.assertEqual(re.findall(r"with_name\('([a-z0-9_-]+\.py)'\)", smoke), ["smoke-check.py", "p9-check.py"])
        menu = (HERE / "ci-sh1-152.py").read_text()
        for name in re.findall(r'"([a-z0-9_-]+(?:-check|_targets)\.py)"', menu):
            self.assertIn(name, SITES)
        self.assertIn("p12-check.py", (HERE / "p12-ci.py").read_text())
        layout = (HERE / "ci-layout-smoke.py").read_text()
        for name in re.findall(r'"(p\d+-check\.py)"', layout):
            self.assertIn(name, SITES)


if __name__ == "__main__":
    unittest.main()
