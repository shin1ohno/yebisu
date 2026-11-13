"""Negative controls for independent failure inputs and exact wire events."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class MenuOracleTests(unittest.TestCase):
    def test_failure_input_current_is_fail_not_unverified(self):
        for acquire, drawing, text in [([False], True, False), ([True], False, False)]:
            row = {"dropped": 0, "status": "Current", "samples": [{"restart_reached": True, "acquire_returns": acquire, "drawing_owned": drawing, "text_owned": text}]}
            with self.assertRaisesRegex(ValueError, "produced switch failure"):
                oracle.check_switch(row)
            row["status"] = "Revoked(SwitchLeftNoResponder)"
            self.assertEqual(oracle.check_switch(row), "pass")

    def test_successful_acquire_and_missing_restart_are_only_unverified(self):
        row = {"dropped": 0, "status": "Current", "samples": [{"restart_reached": True, "acquire_returns": [True], "drawing_owned": False, "text_owned": True}]}
        self.assertIn("unverified", oracle.check_switch(row))
        row["samples"][0].update(restart_reached=False, acquire_returns=[False])
        self.assertIn("unverified", oracle.check_switch(row))
        row["dropped"] = 1
        with self.assertRaisesRegex(ValueError, "dropped"):
            oracle.check_switch(row)

    def test_intervals_refuse_outside_input_and_duplicate_boundaries(self):
        def row(text):
            return {"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Paste": text}]}}
        start, end = row("sh1-152-start-E1-text-0"), row("sh1-152-end-E1-text-0")
        valid = [start, row("payload"), end]
        self.assertEqual(oracle.segments(valid)[("E1-text", 0)]["events"], [("w1:p1", {"Paste": "payload"})])
        for invalid in [[row("outside"), *valid], [start, start, end], [start], [end]]:
            with self.assertRaises(ValueError):
                oracle.segments(invalid)


if __name__ == "__main__":
    unittest.main()
