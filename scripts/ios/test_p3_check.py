import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p3", Path(__file__).with_name("p3-check.py"))
p3 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p3)


class TerminalOracleTests(unittest.TestCase):
    def test_missing_density_and_rotation_receipts_fail(self):
        records = [dict(kind="terminal", boot="boot-v1", projection=7,
                        compact=False, cols=80, rows=24, surface=1, viewport=[100, 200])]
        with self.assertRaisesRegex(ValueError, "both density"):
            p3.validate_records(records, require_rotation=True)
        records.append(dict(records[0], compact=True))
        with self.assertRaisesRegex(ValueError, "rotation/density"):
            p3.validate_records(records, require_rotation=True)
        records += [dict(records[0], cols=120, rows=40, surface=4),
                    dict(records[0], cols=140, rows=30, surface=5, viewport=[200, 100])]
        p3.validate_records(records, require_rotation=True)
        records[0]["boot"] = "old"
        with self.assertRaisesRegex(ValueError, "incoherent"):
            p3.validate_records(records, require_rotation=True)

    def test_dark_blank_screen_is_not_terminal_evidence(self):
        with self.assertRaisesRegex(ValueError, "missing terminal colour"):
            p3.validate_colors({bytes.fromhex("101419"): 1_000_000})
        counts = {bytes.fromhex(c): 100 for c in
                  ("bf616a", "a3be8c", "ebcb8b", "81a1c1", "b48ead", "88c0d0", "00d8de")}
        p3.validate_colors(counts)
        del counts[bytes.fromhex("00d8de")]
        with self.assertRaisesRegex(ValueError, "00d8de"):
            p3.validate_colors(counts)
