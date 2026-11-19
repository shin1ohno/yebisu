"""p3, p9 and p10 (S2) and the edit-menu job's sh1-152, sh1-153, p8-native, selection-settings and
sh1-156 (S4) read the app's files only after the app was terminated (UI test redesign).

They used to read p0-smoke.jsonl, p9-core-wire.jsonl and p10-settings.jsonl right after XCTest,
with the app possibly still writing (plan 1.3: the same risk that failed p2 three times on 11-18)."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
DRIVERS = {"p3-check.py": "P3", "p9-check.py": "P9", "p10-check.py": "P10", "sh1-153-check.py": "SH1-153",
           "p8-native-check.py": "P8", "selection-settings-check.py": "SH1-151", "sh1-156-check.py": "SH1-156",
           "sh1-152-check.py": "SH1-152"}


def load(file):
    spec = importlib.util.spec_from_file_location(file.replace("-", "_")[:-3], HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main_text(file):
    text = (HERE / file).read_text()
    return text[text.index("def main():"):]


class OrderTests(unittest.TestCase):
    def test_xctest_then_terminate_then_the_first_read(self):
        reads = {"p3-check.py": "collect(result", "p9-check.py": "collect_evidence(", "p10-check.py": "observations.read_text().splitlines()[previous:]",
                 "sh1-153-check.py": "shutil.copyfile(container", "p8-native-check.py": "shutil.copyfile(container",
                 "selection-settings-check.py": "live_jsonl.read_records(observations", "sh1-156-check.py": "shutil.copyfile(container",
                 "sh1-152-check.py": "shutil.copyfile(path, directory"}
        for file, read in reads.items():
            with self.subTest(driver=file):
                text = main_text(file)
                xctest = text.index('"xcodebuild"') if '"xcodebuild"' in text else text.index("xcodebuild_test_command(")
                after = text.index("terminate(device)", xctest)
                self.assertLess(after, text.index(read, xctest), "the read must follow the terminate")
                self.assertNotIn('run("xcrun", "simctl", "terminate"', text, "the old terminate at the end is gone")

    def test_the_terminate_runs_also_when_xctest_fails(self):
        for file in DRIVERS:
            with self.subTest(driver=file):
                text = main_text(file)
                at = text.index("terminate(device)", text.index("xcodebuild"))
                self.assertIn("finally:", text[text.rindex("\n", 0, at) - 120:at])

    def test_baseline_counts_are_read_with_no_app_running(self):
        for file, baseline in {"p9-check.py": "previous = len(", "p10-check.py": "previous = len(",
                               "selection-settings-check.py": "previous = live_jsonl.count_lines(",
                               "sh1-153-check.py": '(container / "Documents/p4-peer.jsonl").unlink(',
                               "p8-native-check.py": '(container / "Documents/p4-peer.jsonl").unlink(',
                               "sh1-156-check.py": '(container / "Documents" / name).unlink(',
                               "sh1-152-check.py": "path.unlink(missing_ok=True)"}.items():
            with self.subTest(driver=file):
                text = main_text(file)
                self.assertLess(text.index("terminate(device)"), text.index(baseline))


class TerminateTests(unittest.TestCase):
    def call(self, module, code):
        completed = subprocess.CompletedProcess([], code, "", "" if code == 0 else "simctl said no")
        with mock.patch.object(module.subprocess, "run", return_value=completed) as run, contextlib.redirect_stdout(io.StringIO()) as out:
            module.terminate("dev")
        self.assertEqual(run.call_args.args[0], ["xcrun", "simctl", "terminate", "dev", "com.shin1ohno.yebisu"])
        return out.getvalue()

    def test_running_and_already_exited_apps_pass(self):
        for file, tag in DRIVERS.items():
            with self.subTest(driver=file):
                module = load(file)
                self.assertIn(tag + " terminate before reading the app's files: exit 0", self.call(module, 0))
                self.assertIn("exit 3", self.call(module, 3))

    def test_negative_another_failure_stops_before_the_read(self):
        for file in DRIVERS:
            with self.subTest(driver=file):
                with self.assertRaisesRegex(RuntimeError, "could not terminate the app before reading its files"):
                    self.call(load(file), 1)


if __name__ == "__main__":
    unittest.main()
