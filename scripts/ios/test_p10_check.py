import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p10", Path(__file__).with_name("p10-check.py"))
p10 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p10)


class P10Oracle(unittest.TestCase):
    def test_valid_and_negative_receiving_shapes(self):
        saved = json.loads((Path(__file__).parent / "fixtures/settings-writer-default.json").read_text())
        saved.update(compact=True, terminal_priority=True)
        loaded = [dict(kind="settings-loaded", compact=False, terminal_priority=False),
                  dict(kind="settings-loaded", compact=True, terminal_priority=True)]
        lifecycle = [dict(kind="settings-lifecycle", phase=p) for p in ["Inactive", "Background", "Active"]]
        p10.validate(loaded + lifecycle, saved)
        with self.assertRaisesRegex(ValueError, "file differs"):
            p10.validate(loaded + lifecycle, dict(saved, compact=False))
        with self.assertRaisesRegex(ValueError, "UIKit"):
            p10.validate(loaded, saved)
        with self.assertRaisesRegex(ValueError, "fresh process"):
            p10.validate(loaded[:1] + lifecycle, saved)

        for key, value in [("route", "system"), ("sound", False), ("volume", 51), ("muted", True), ("quiet", [0, 1])]:
            with self.subTest(notification_field=key), self.assertRaisesRegex(ValueError, "file differs"):
                p10.validate(loaded + lifecycle, dict(saved, notifications=dict(saved["notifications"], **{key: value})))
        missing = dict(saved)
        del missing["notifications"]
        with self.assertRaisesRegex(ValueError, "file differs"):
            p10.validate(loaded + lifecycle, missing)
