import importlib.util
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("sh1_156", Path(__file__).with_name("sh1-156-check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


def commits(text):
    return {"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"TextCommit": c} for c in text]}}


class SH1156OracleTests(unittest.TestCase):
    def fixture(self):
        records = [{"kind": "sh1-156-discarded", "marked": "てすと", "fenced": True,
                    "drag_select": True, "select_mode": False},
                   {"kind": "keyboard-request", "reason": "settings-close"}]
        return records, [commits("clicked"), commits("released"), commits("settings")]

    def test_expected(self):
        check.validate(*self.fixture())

    def test_plain_keys_count_as_letters(self):
        records, _ = self.fixture()
        keys = [{"Key": {"code": {"Char": c}, "modifiers": 0, "kind": kind}}
                for c in "clickedreleasedsettings" for kind in ["Press", "Release"]]
        check.validate(records, [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": keys}}])

    def test_letters_dropped_after_click(self):
        # c62cb33: the click left a one-cell selection that dropped every letter.
        records, messages = self.fixture()
        with self.assertRaisesRegex(ValueError, "letters after the click"):
            check.validate(records, messages[1:])

    def test_letters_dropped_after_select_off(self):
        records, messages = self.fixture()
        with self.assertRaisesRegex(ValueError, "letters after the click"):
            check.validate(records, messages[:1])

    def test_letters_dropped_after_settings(self):
        # R7: after Settings closed, letters typed without a touch were lost.
        records, messages = self.fixture()
        with self.assertRaisesRegex(ValueError, "letters after the click"):
            check.validate(records, messages[:2])

    def test_settings_close_must_request_keyboard(self):
        records, messages = self.fixture()
        with self.assertRaisesRegex(ValueError, "did not ask for the keyboard"):
            check.validate(records[:1], messages)

    def test_discarded_commit_leak(self):
        records, messages = self.fixture()
        with self.assertRaisesRegex(ValueError, "discarded composition reached"):
            check.validate(records, [commits("てすと")] + messages)

    def test_fixture_must_fence(self):
        records, messages = self.fixture()
        records[0]["fenced"] = False
        with self.assertRaisesRegex(ValueError, "did not leave a discarded composition"):
            check.validate(records, messages)

    def test_fixture_must_start_with_drag(self):
        records, messages = self.fixture()
        records[0]["drag_select"] = False
        with self.assertRaisesRegex(ValueError, "Start: Drag"):
            check.validate(records, messages)


if __name__ == "__main__":
    unittest.main()
