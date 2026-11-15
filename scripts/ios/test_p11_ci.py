"""The hosted wrapper may pass only when all four collector cases ran as designed."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("p11_ci", Path(__file__).with_name("p11-ci.py"))
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


def artifacts(cases, accepted=False):
    directory = Path(tempfile.mkdtemp())
    (directory / "source-candidate-controls.json").write_text(
        json.dumps([{"case": name, "observed_controls": True} for name in cases]))
    (directory / "acceptance.json").write_text(json.dumps({"accepted": accepted}))
    return directory


class Verdict(unittest.TestCase):
    def test_designed_exit_with_four_cases_passes(self):
        self.assertIsNone(ci.verdict(1, ci.CLOSING, artifacts(ci.CASES)))

    def test_other_exit_or_message_fails(self):
        self.assertIsNotNone(ci.verdict(0, ci.CLOSING, artifacts(ci.CASES)))
        self.assertIsNotNone(ci.verdict(1, "Traceback", artifacts(ci.CASES)))

    def test_missing_or_reordered_case_fails(self):
        self.assertIsNotNone(ci.verdict(1, ci.CLOSING, artifacts(ci.CASES[:3])))
        self.assertIsNotNone(ci.verdict(1, ci.CLOSING, artifacts(list(reversed(ci.CASES)))))

    def test_accepted_true_fails(self):
        self.assertIsNotNone(ci.verdict(1, ci.CLOSING, artifacts(ci.CASES, accepted=True)))

    def test_missing_artifacts_fail(self):
        self.assertIsNotNone(ci.verdict(1, ci.CLOSING, Path(tempfile.mkdtemp())))


if __name__ == "__main__":
    unittest.main()
