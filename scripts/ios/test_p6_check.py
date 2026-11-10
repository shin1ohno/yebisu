import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p6", Path(__file__).with_name("p6-check.py"))
p6 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p6)

class BoardOracleTests(unittest.TestCase):
    def records(self):
        return [dict(kind="board", host=host, selected=selected, revision=revision,
                     states=["Working" if revision % 2 else "Blocked"], rows=["[Claude] reviewer"])
                for host in ["qa-alpha", "qa-beta"] for selected in [True, False]
                for revision in [7, 8, 9]]

    def test_positive(self):
        p6.validate(self.records())

    def test_hidden_stall_fails(self):
        records = self.records()
        for r in records:
            if not r["selected"]:
                r["revision"] = 7
        with self.assertRaisesRegex(ValueError, "hidden host"):
            p6.validate(records)

    def test_missing_host_fails(self):
        with self.assertRaisesRegex(ValueError, "host did not update"):
            p6.validate([r for r in self.records() if r["host"] == "qa-alpha"])

    def test_wrong_status_fails(self):
        records = self.records()
        for r in records:
            r["states"] = ["Idle"]
        with self.assertRaisesRegex(ValueError, "state transitions"):
            p6.validate(records)

    def test_missing_harness_fails(self):
        records = self.records()
        for r in records:
            r["rows"] = ["reviewer"]
        with self.assertRaisesRegex(ValueError, "harness mark"):
            p6.validate(records)
