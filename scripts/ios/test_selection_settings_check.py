import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("selection_settings", Path(__file__).with_name("selection-settings-check.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class SelectionSettingsOracle(unittest.TestCase):
    def test_primary_expectations_and_negative_controls(self):
        records = [dict(kind="settings-loaded", drag_select=True, copy_on_select=False, terminal_font_size=None, terminal_font_family="SauceCodePro Nerd Font Mono"),
                   dict(kind="settings-loaded", drag_select=False, copy_on_select=True, terminal_font_size=None, terminal_font_family="SauceCodePro Nerd Font Mono")]
        saved = json.loads((Path(__file__).parent / "fixtures/settings-writer-default.json").read_text())
        saved.update(drag_select=False, copy_on_select=True)
        oracle.validate(records, saved)
        with self.assertRaisesRegex(ValueError, "fresh defaults"):
            oracle.validate([dict(records[0], drag_select=False), records[1]], saved)
        with self.assertRaisesRegex(ValueError, "relaunch"):
            oracle.validate([records[0], dict(records[1], copy_on_select=False, terminal_font_size=None, terminal_font_family="SauceCodePro Nerd Font Mono")], saved)
        with self.assertRaisesRegex(ValueError, "receiving file"):
            oracle.validate(records, dict(saved, board_hidden=True))
        with self.assertRaisesRegex(ValueError, "two process"):
            oracle.validate(records[:1], saved)

        for key, value in [("route", "system"), ("sound", False), ("volume", 51), ("muted", True), ("quiet", [0, 1])]:
            with self.subTest(notification_field=key), self.assertRaisesRegex(ValueError, "receiving file"):
                oracle.validate(records, dict(saved, notifications=dict(saved["notifications"], **{key: value})))
        missing = dict(saved)
        del missing["notifications"]
        with self.assertRaisesRegex(ValueError, "receiving file"):
            oracle.validate(records, missing)
