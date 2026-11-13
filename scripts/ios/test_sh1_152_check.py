"""Negative controls for independent failure inputs and exact wire events."""
import importlib.util
import copy
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class MenuOracleTests(unittest.TestCase):
    def fixture(self):
        records, messages, wire = [], [], []
        for case in oracle.BASIC_CASES:
            copying = case.startswith("E2")
            payload = f"sh1-152-clip-{case}-0"
            records.extend([
                {"case": case, "kind": "sh1-152-capture", "count": 2, "time": 1},
                {"case": case, "kind": "edit-menu-exec", "result": "executed", "action": "Copy" if copying else "Paste", "time": 2},
                {"case": case, "kind": "sh1-152-result", "clipboard": "日本語" if copying else payload, "time": 3},
            ])
            values = [f"sh1-152-start-{case}-0", *([] if copying else [payload]), f"sh1-152-end-{case}-0"]
            if case.endswith("core"):
                if not copying:
                    records.append({"case": case, "kind": "terminal-input", "variant": "Paste"})
                wire.extend({"terminal": "core-A", "bytes": list(b"\x1b[200~" + value.encode() + b"\x1b[201~")} for value in values)
            else:
                messages.extend({"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Paste": value}]}} for value in values)
        return records, messages, wire

    def test_real_shape_positive_and_same_text_key_wrong_pane_outside_interval_negatives(self):
        fixture = self.fixture()
        self.assertIn("PASS", oracle.validate_basic(*fixture))
        for mutation in ["key", "pane", "outside", "duplicate", "clipboard", "core-key", "core-pane", "core-byte"]:
            records, messages, wire = copy.deepcopy(fixture)
            if mutation == "key":
                messages[1]["ClientShellPaneInput"]["events"] = [{"Key": {"code": {"Char": "sh1-152-clip-E1-text-0"}}}]
            elif mutation == "pane":
                messages[1]["ClientShellPaneInput"]["pane_id"] = "w1:p2"
            elif mutation == "outside":
                row = next(r for r in records if r.get("kind") == "edit-menu-exec")
                row["time"] = 4
            elif mutation == "duplicate":
                records.append(next(r for r in records if r.get("kind") == "edit-menu-exec"))
            elif mutation == "clipboard":
                next(r for r in records if r.get("kind") == "sh1-152-result")["clipboard"] = "wrong"
            elif mutation == "core-key":
                next(r for r in records if r.get("kind") == "terminal-input")["variant"] = "Key"
            elif mutation == "core-pane":
                wire[1]["terminal"] = "core-B"
            else:
                wire[1]["bytes"] = list(b"sh1-152-clip-E1-core-0")
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                oracle.validate_basic(records, messages, wire)

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
