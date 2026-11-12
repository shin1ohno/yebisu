"""Source-defined lifecycle receipts; reject each broken lifecycle contract."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("sh1", Path(__file__).with_name("sh1-155-check.py"))
sh1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sh1)


class InactiveOracle(unittest.TestCase):
    def records(self):
        return [
            dict(kind="lifecycle", phase="Inactive", generation=1, suspended=False, connected=True, herdr1=True),
            dict(kind="lifecycle", phase="Active", generation=1, suspended=False, connected=True, herdr1=True),
            dict(kind="lifecycle", phase="Background", generation=2, suspended=True, connected=False, herdr1=False),
            dict(kind="reconnect", generation=3),
            dict(kind="terminal", generation=3),
        ]

    def test_valid_receipts(self):
        sh1.validate(self.records())

    def test_each_contract_has_a_negative_control(self):
        records = self.records()
        for index, key, value, reason in [
            (0, "herdr1", False, "lost"),
            (1, "generation", 2, "reconnected"),
            (2, "herdr1", True, "retained"),
            (3, "generation", 2, "did not reconnect"),
            (4, "generation", 1, "did not paint"),
        ]:
            with self.subTest(reason=reason):
                broken = copy.deepcopy(records)
                broken[index][key] = value
                with self.assertRaisesRegex(ValueError, reason):
                    sh1.validate(broken)
        with self.assertRaisesRegex(ValueError, "missing native"):
            sh1.validate(records[2:])
