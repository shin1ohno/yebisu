"""R5 observation oracle: compatible hidden reconnects cannot mask a bad dial."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("r5_check", Path(__file__).with_name("r5-check.py"))
r5 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r5)


class R5Check(unittest.TestCase):
    def records(self):
        return [
            {"kind": "lifecycle", "phase": "Background", "generation": 2},
            {"kind": "reconnect", "host": "compatible-hidden", "generation": 1},
            {"kind": "r5-reader-release", "pending": True, "automatic": True, "generation": 2},
            {"kind": "lifecycle", "phase": "Active", "generation": 2},
        ]

    def test_compatible_hidden_reconnect_is_allowed(self):
        r5.validate(self.records())

    def test_incompatible_selected_redial_is_rejected(self):
        records = self.records()
        records.insert(1, {"kind": "reconnect", "host": "qa@private-peer"})
        with self.assertRaisesRegex(ValueError, "redialed"):
            r5.validate(records)

    def test_manual_or_unblocked_return_is_rejected(self):
        for field in ["automatic", "pending"]:
            records = self.records()
            records[2][field] = False
            with self.assertRaisesRegex(ValueError, "held through Automatic"):
                r5.validate(records)

    def test_missing_or_early_reader_release_is_rejected(self):
        records = self.records()
        with self.assertRaisesRegex(ValueError, "held through Automatic"):
            r5.validate([r for r in records if r["kind"] != "r5-reader-release"])
        records.insert(0, records.pop(2))
        with self.assertRaisesRegex(ValueError, "straddle"):
            r5.validate(records)

    def test_changed_generation_is_rejected(self):
        records = copy.deepcopy(self.records())
        records[-1]["generation"] += 1
        with self.assertRaisesRegex(ValueError, "straddle"):
            r5.validate(records)


if __name__ == "__main__":
    unittest.main()
