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


class NavigationOracleTests(unittest.TestCase):
    def records(self):
        records = []
        for before, after, rev in [("w1:p1", "w2:p2", 8), ("w2:p2", "w1:p1", 10)]:
            common = dict(host="qa-beta", boot="boot-v1", generation=1)
            records.extend([dict(kind="terminal", pane=before, **common),
                            dict(kind="navigation", pane=after, revision=rev, **common),
                            dict(kind="terminal", pane=after, projection=rev+1, input_pane=after,
                                 surface_marker="2" if after == "w2:p2" else "1", **common)])
        return records

    def test_positive(self):
        p6.validate_navigation(self.records())

    def test_peer_ignores_focus(self):
        records = self.records()
        records[2]["pane"] = "w1:p1"
        with self.assertRaisesRegex(ValueError, "did not move"):
            p6.validate_navigation(records)

    def test_peer_wrong_target(self):
        records = self.records()
        records[2]["pane"] = "wrong"
        with self.assertRaisesRegex(ValueError, "did not move"):
            p6.validate_navigation(records)

    def test_peer_stale_boot(self):
        records = self.records()
        records[2]["boot"] = "stale"
        with self.assertRaisesRegex(ValueError, "did not move"):
            p6.validate_navigation(records)

    def test_unchanged_surface(self):
        records = self.records()
        records[2]["surface_marker"] = "1"
        with self.assertRaisesRegex(ValueError, "did not move"):
            p6.validate_navigation(records)

    def test_unchanged_input(self):
        records = self.records()
        records[2]["input_pane"] = "w1:p1"
        with self.assertRaisesRegex(ValueError, "did not move"):
            p6.validate_navigation(records)
