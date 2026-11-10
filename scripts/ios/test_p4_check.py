import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p4", Path(__file__).with_name("p4-check.py"))
p4 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p4)


class InputOracle(unittest.TestCase):
    def records(self):
        return [dict(kind="terminal", marked="かな😀変換", selected=[2, 4], time=1),
                dict(kind="terminal", marked="", selected=[0, 0], time=2)]

    def messages(self, text="P4-onceかな😀変換", target="w1:p1"):
        return [{"ClientShellPaneInput": {"pane_id": target, "events": [{"TextCommit": text}]}}]

    def test_accepts_specified_receipt(self):
        p4.validate(self.records(), self.messages())

    def test_duplicate_input_fails_for_duplicate_text(self):
        with self.assertRaisesRegex(ValueError, "duplicated"):
            p4.validate(self.records(), self.messages("P4-onceP4-onceかな😀変換"))

    def test_wrong_utf16_clause_fails_for_selection(self):
        records = self.records()
        records[0]["selected"] = [6, 6]
        with self.assertRaisesRegex(ValueError, "UTF-16 clause"):
            p4.validate(records, self.messages())

    def test_wrong_target_fails_for_target(self):
        with self.assertRaisesRegex(ValueError, "wrong pane"):
            p4.validate(self.records(), self.messages(target="other"))

    def test_missing_clear_fails_for_commit(self):
        with self.assertRaisesRegex(ValueError, "clear rendered composition"):
            p4.validate(self.records()[:1], self.messages())
