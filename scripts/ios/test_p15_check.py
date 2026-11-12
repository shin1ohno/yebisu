import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p15", Path(__file__).with_name("p15-check.py"))
p15 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p15)


class ThemeOracle(unittest.TestCase):
    def fixture(self):
        def paint(theme="high-contrast", modal=False):
            return dict(kind="terminal", theme=theme, settings_modal=modal)
        records = [dict(kind="settings-loaded", theme="owner-default"),
                   paint("owner-default"), dict(kind="theme-selected", theme="high-contrast"),
                   paint(modal=True), paint(),
                   dict(kind="settings-lifecycle", phase="Background"),
                   dict(kind="settings-lifecycle", phase="Active"), paint(),
                   dict(kind="settings-loaded", theme="high-contrast"), paint()]
        return records, dict(copy.deepcopy(p15.SEED), theme="high-contrast")

    def test_primary_receiving_shape(self):
        p15.validate(*self.fixture())

    def test_wrong_or_missing_saved_field(self):
        for field in p15.SEED:
            records, disk = self.fixture()
            disk.pop(field)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "exact saved"):
                p15.validate(records, disk)
        records, disk = self.fixture()
        disk["theme"] = "owner-default"
        with self.assertRaisesRegex(ValueError, "exact saved"):
            p15.validate(records, disk)
        disk["theme"] = "high-contrast"
        disk["notifications"]["volume"] = 38
        with self.assertRaisesRegex(ValueError, "exact saved"):
            p15.validate(records, disk)

    def test_missing_real_click_lifecycle_load_and_paints(self):
        for index, reason in [(0, "two real"), (2, "Appearance"),
                              (3, "immediate modal"), (4, "closed-sheet"),
                              (5, "background/return"), (6, "background/return"),
                              (7, "background return"), (8, "two real"),
                              (9, "relaunched painter")]:
            records, disk = self.fixture()
            records.pop(index)
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, reason):
                p15.validate(records, disk)

    def test_stale_theme_paint_is_not_success(self):
        records, disk = self.fixture()
        records[3]["theme"] = "owner-default"
        with self.assertRaisesRegex(ValueError, "immediate modal"):
            p15.validate(records, disk)


if __name__ == "__main__":
    unittest.main()
