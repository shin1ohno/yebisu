"""input-mode-check.py's overall limit on the test run (#212), with stand-in commands for xcodebuild."""
import importlib.util
import json
from pathlib import Path
import signal
import subprocess
import sys
import time
import unittest

spec = importlib.util.spec_from_file_location("input_mode_check", Path(__file__).with_name("input-mode-check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


def obs_line(step):
    return "INPUT_MODE_OBS " + json.dumps({"step": step, "input_mode": {}})


def child(body):
    """A Python process running `body`, flushing every print, as xcodebuild's stand-in."""
    return [sys.executable, "-I", "-c", "import functools, signal, sys, time\n"
            "print = functools.partial(print, flush=True)\n" + body]


class RunTestTests(unittest.TestCase):
    def timed(self, *args, **kwargs):
        started = time.monotonic()
        result = check.run_test(*args, **kwargs)
        return result, time.monotonic() - started

    def test_a_test_that_ends_by_itself_reads_as_subprocess_run_did(self):
        command = child("print('out 1'); print('err 1', file=sys.stderr); print('" + obs_line("s1-focused")
                        + "'); sys.exit(65)")
        expected = subprocess.run(command, text=True, capture_output=True)
        (code, out, err, stopped), _ = self.timed(command, None, timeout=30, stop_after="s2-h2-after")
        self.assertEqual((code, out, err), (expected.returncode, expected.stdout, expected.stderr))
        self.assertEqual(code, 65)
        self.assertIsNone(stopped)

    def test_the_limit_terminates_a_test_that_never_ends_and_keeps_its_output(self):
        command = child("print('" + obs_line("s1-switch-1") + "'); time.sleep(60)")
        (code, out, _, stopped), took = self.timed(command, None, timeout=1, grace=10)
        self.assertLess(took, 10)
        self.assertEqual(code, -signal.SIGTERM)
        self.assertEqual(stopped["reason"], "timeout")
        self.assertGreaterEqual(stopped["after_s"], 1)
        self.assertIn("s1-switch-1", check.observations(out))

    def test_the_named_step_stops_the_test_and_another_step_does_not(self):
        command = child("print('" + obs_line("s2-h1-after") + "'); time.sleep(1.5); print('"
                        + obs_line("s2-h2-after") + "'); time.sleep(60)")
        (code, out, _, stopped), took = self.timed(command, None, timeout=30, stop_after="s2-h2-after", grace=10)
        self.assertLess(took, 10)
        self.assertEqual(code, -signal.SIGTERM)
        self.assertEqual(stopped["reason"], "step")
        self.assertEqual(stopped["step"], "s2-h2-after")
        self.assertGreaterEqual(stopped["after_s"], 1.5)
        self.assertEqual(sorted(check.observations(out)), ["s2-h1-after", "s2-h2-after"])
        # The step's name in other output, or another step, is not the step's observation.
        command = child("print('waiting for s2-h2-after'); print('" + obs_line("s2-h1-after") + "'); time.sleep(1)")
        (code, _, _, stopped), _ = self.timed(command, None, timeout=30, stop_after="s2-h2-after")
        self.assertEqual((code, stopped), (0, None))

    def test_a_test_that_ignores_sigterm_is_killed_after_the_grace(self):
        command = child("signal.signal(signal.SIGTERM, signal.SIG_IGN); print('ready'); time.sleep(60)")
        (code, out, _, stopped), took = self.timed(command, None, timeout=1, grace=1)
        self.assertLess(took, 10)
        self.assertEqual(code, -signal.SIGKILL)
        self.assertEqual(stopped["reason"], "timeout")
        self.assertEqual(out, "ready\n")


if __name__ == "__main__":
    unittest.main()
