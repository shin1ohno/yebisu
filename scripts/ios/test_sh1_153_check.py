import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("edit", Path(__file__).with_name("sh1-153-check.py"))
edit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(edit)

class EditMenuOracleTests(unittest.TestCase):
    def fixture(self):
        records, messages = [], []
        for case in range(10):
            enabled = case % 5 in (0, 1, 4)
            records.extend([
                {"kind":"edit-validation", "case":case, "probe":{
                    "enabled":enabled, "validated":enabled,
                    "responder":"GPUITextInputView" if case < 5 and case % 5 != 3 else "HerdrUIKitGPUIView"}},
                {"kind":"edit-result", "case":case,
                    "clipboard":"日本語" if case % 5 == 1 else f"edit-text-{case}"}])
            events = [{"Paste":f"edit-start-{case}"}]
            if case % 5 == 0:
                events.append({"Paste":f"edit-text-{case}"})
            events.append({"Paste":f"edit-end-{case}"})
            messages.append({"ClientShellPaneInput":{"events":events}})
        return records, messages

    def test_specification_fixture(self):
        edit.validate(*self.fixture())

    def test_rejects_original_mode_gate_and_missing_validation(self):
        for case, field in [(5,"enabled"),(0,"validated"),(6,"enabled")]:
            records, messages = self.fixture()
            records[case*2]["probe"][field] = False
            with self.assertRaisesRegex(ValueError, "enabling mismatch"):
                edit.validate(records, messages)

    def test_rejects_key_leak_missing_paste_and_stale_copy(self):
        for case in (0, 5):
            records, messages = self.fixture()
            messages[case]["ClientShellPaneInput"]["events"][1] = {"Key":{"kind":"Press"}}
            with self.assertRaisesRegex(ValueError, "terminal input mismatch"):
                edit.validate(records, messages)
        records, messages = self.fixture()
        records[9]["clipboard"] = "日本語"
        with self.assertRaisesRegex(ValueError, "clipboard mismatch"):
            edit.validate(records, messages)

    def test_rejects_empty_copy_unfocused_and_wrong_responder(self):
        for case in (2,3,7,8):
            records, messages = self.fixture()
            records[case*2]["probe"]["enabled"] = True
            with self.assertRaisesRegex(ValueError, "enabling mismatch"):
                edit.validate(records, messages)
        records, messages = self.fixture()
        records[10]["probe"]["responder"] = "GPUITextInputView"
        with self.assertRaisesRegex(ValueError, "wrong keyboard responder"):
            edit.validate(records, messages)

    def test_rejects_truncated_evidence(self):
        records, messages = self.fixture()
        with self.assertRaisesRegex(ValueError, "incomplete"):
            edit.validate(records, messages[:-1])
        with self.assertRaisesRegex(ValueError, "cases missing"):
            edit.validate(records[:-1], messages)
