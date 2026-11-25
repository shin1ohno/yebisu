"""Exercise scripts/run-python-tests.py on throwaway test directories."""
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest

REPO = Path(__file__).resolve().parents[2]
RUNNER = REPO / "scripts/run-python-tests.py"

PASSING = """
    import unittest
    class T(unittest.TestCase):
        def test_one(self): pass
        def test_two(self): pass
"""
FAILING = """
    import unittest
    class T(unittest.TestCase):
        def test_fine(self): pass
        def test_broken(self): self.assertEqual(1, 2, "FAILURE-MARKER")
"""


def write(directory, name, body):
    path = Path(directory) / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(textwrap.dedent(body))


def run(*dirs, jobs="4"):
    return subprocess.run([sys.executable, "-I", "-B", str(RUNNER), "--jobs", jobs, *map(str, dirs)],
                          capture_output=True, text=True, timeout=120)


class RunnerTests(unittest.TestCase):
    def test_passing_modules_in_two_directories_report_the_discovered_total(self):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            write(a, "test_a.py", PASSING)
            write(a, "helper.py", "VALUE = 1\n")  # not a test module
            write(b, "test_b.py", PASSING)
            write(b, "test_c.py", PASSING)
            result = run(a, b)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Ran 6 tests in 3 modules", result.stdout)
        self.assertIn("discovery finds 6", result.stdout)
        self.assertRegex(result.stdout, r"(?m)^ok   .*/test_b\.py: 2 tests in [0-9.]+ s$")
        self.assertTrue(result.stdout.rstrip().endswith("OK"))

    def test_a_failing_test_fails_the_run_and_prints_its_traceback(self):
        with tempfile.TemporaryDirectory() as a:
            write(a, "test_good.py", PASSING)
            write(a, "test_bad.py", FAILING)
            result = run(a)
        self.assertEqual(result.returncode, 1)
        self.assertRegex(result.stdout, r"(?m)^FAIL .*/test_bad\.py: 2 tests in ")
        self.assertRegex(result.stdout, r"(?m)^ok   .*/test_good\.py: 2 tests in ")
        self.assertIn("FAILURE-MARKER", result.stderr)
        self.assertIn("test_broken", result.stderr)
        self.assertRegex(result.stderr, r"FAILED: 1 of 2 modules: .*/test_bad\.py")

    def test_a_module_that_cannot_be_imported_fails_the_run(self):
        with tempfile.TemporaryDirectory() as a:
            write(a, "test_good.py", PASSING)
            write(a, "test_import.py", "import no_such_module_anywhere\n")
            result = run(a)
        self.assertEqual(result.returncode, 1)
        self.assertIn("no_such_module_anywhere", result.stderr)
        self.assertRegex(result.stdout, r"(?m)^FAIL .*/test_import\.py")

    def test_tests_that_per_module_runs_miss_fail_the_count_check(self):
        # Whole-directory discovery enters packages; the per-module runs do not.
        with tempfile.TemporaryDirectory() as a:
            write(a, "test_top.py", PASSING)
            write(a, "pkg/__init__.py", "")
            write(a, "pkg/test_inner.py", PASSING)
            result = run(a)
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAILED: ran 2 tests, but discovery finds 4", result.stderr)

    def test_one_job_still_runs_every_module(self):
        with tempfile.TemporaryDirectory() as a:
            for name in ("test_x.py", "test_y.py", "test_z.py"):
                write(a, name, PASSING)
            result = run(a, jobs="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 6 tests in 3 modules", result.stdout)

    def test_zero_jobs_is_refused(self):
        with tempfile.TemporaryDirectory() as a:
            result = run(a, jobs="0")
        self.assertEqual(result.returncode, 2)
        self.assertIn("--jobs must be at least 1", result.stderr)

    def test_just_test_runs_both_directories_through_the_runner(self):
        recipe = re.search(r"(?m)^test:\n((?:    [^\n]*\n)+)", (REPO / "justfile").read_text()).group(1)
        self.assertIn("python3 -I -B scripts/run-python-tests.py scripts/tests scripts/ios\n", recipe)
        self.assertNotIn("-m unittest", recipe)


if __name__ == "__main__":
    unittest.main()
