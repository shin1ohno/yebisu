"""Issue #261: the edit-menu job keeps the crash reports its Simulator run wrote."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("crash_reports", REPO / "scripts/ios/crash_reports.py")
crash_reports = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash_reports)


class CopySince(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "DiagnosticReports"
        self.source.mkdir()
        self.destination = self.root / "evidence" / "crash-reports"

    def report(self, name, age):
        path = self.source / name
        path.write_text('{"app_name":"SpringBoard","platform":7}\n{}\n')
        stamp = time.time() - age
        os.utime(path, (stamp, stamp))
        return path

    def test_only_reports_written_since_the_start_are_copied(self):
        self.report("SpringBoard-2026-11-27-090225.ips", age=10)
        self.report("yebisu-2026-11-27-090300.ips", age=5)
        self.report("SpringBoard-2026-11-20-120000.ips", age=3600)  # an older run's report
        (self.source / "notes.txt").write_text("not a report")
        copied = crash_reports.copy_since(time.time() - 60, self.destination, self.source)
        self.assertEqual(copied, ["SpringBoard-2026-11-27-090225.ips", "yebisu-2026-11-27-090300.ips"])
        self.assertEqual(sorted(p.name for p in self.destination.iterdir()), copied)
        self.assertIn('"platform":7', (self.destination / copied[0]).read_text())

    def test_no_new_report_creates_nothing(self):
        # Negative control: an old report alone neither copies nor creates the directory.
        self.report("SpringBoard-2026-11-20-120000.ips", age=3600)
        self.assertEqual(crash_reports.copy_since(time.time() - 60, self.destination, self.source), [])
        self.assertFalse(self.destination.exists())

    def test_a_missing_reports_directory_copies_nothing(self):
        missing = self.root / "absent"
        self.assertEqual(crash_reports.copy_since(0, self.destination, missing), [])

    def test_an_unreadable_report_is_named_and_the_rest_are_copied(self):
        locked = self.report("SpringBoard-a.ips", age=1)
        self.report("SpringBoard-b.ips", age=1)
        locked.chmod(0)
        self.addCleanup(locked.chmod, 0o644)
        copied = crash_reports.copy_since(time.time() - 60, self.destination, self.source)
        self.assertEqual(len(copied), 2)
        self.assertTrue(copied[0].startswith("SpringBoard-a.ips (skipped: "), copied)
        self.assertEqual(copied[1], "SpringBoard-b.ips")


class SuiteWiring(unittest.TestCase):
    def test_the_edit_menu_suite_copies_reports_before_deleting_its_simulator(self):
        source = (REPO / "scripts/ios/ci-sh1-152.py").read_text()
        finally_block = source.split("    finally:\n", 1)[1]
        self.assertLess(finally_block.index("crash_reports.copy_since(started, EVIDENCE)"),
                        finally_block.index('"simctl", "delete"'))
        self.assertIn('"target" / "ios-sh1-152" / "crash-reports"', source)
        # The uploaded edit-menu evidence includes that directory.
        workflow = (REPO / ".github/workflows/ci.yml").read_text()
        evidence = workflow.split("name: edit-menu-evidence", 1)[1].split("\n  simulator-smoke:", 1)[0]
        self.assertIn("target/ios-sh1-152/", evidence)

    def test_the_suite_module_loads(self):
        result = subprocess.run([sys.executable, "-I", "-c",
                                 "import importlib.util,sys;p=sys.argv[1];s=importlib.util.spec_from_file_location('m',p);"
                                 "m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(m.EVIDENCE)",
                                 str(REPO / "scripts/ios/ci-sh1-152.py")], capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.strip().endswith("target/ios-sh1-152/crash-reports"), result.stdout)


if __name__ == "__main__":
    unittest.main()
