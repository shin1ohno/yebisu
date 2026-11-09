"""The receiving-system oracle must reject missing/stale work."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p5", Path(__file__).with_name("p5-check.py"))
p5 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p5)

class P5Oracle(unittest.TestCase):
    def records(self):
        return [
            {"kind": "reconnect", "generation": 3, "automatic": True},
            {"kind": "terminal", "generation": 3, "boot": "boot-v1"},
            {"kind": "reconnect", "generation": 5, "automatic": False},
            {"kind": "terminal", "generation": 5, "boot": "boot-v1"},
            {"kind": "setup", "hosts": ["qa@private-one", "qa@private-two"]},
            {"kind": "setup", "status": "Developer-injected save failure"},
        ]
    def test_positive(self):
        p5.validate(self.records())
    def test_missing_each_receipt_fails(self):
        for index in range(6):
            records = self.records()
            del records[index]
            with self.assertRaises(ValueError):
                p5.validate(records)
    def test_old_generation_and_wrong_boot_fail(self):
        for key, value in [("generation", 1), ("boot", "old")]:
            records = self.records()
            records[3][key] = value
            with self.assertRaises(ValueError):
                p5.validate(records)

    def test_hidden_host_connections_do_not_count_as_selected_reconnects(self):
        records = self.records() + [
            {"kind": "reconnect", "generation": 1, "automatic": True,
             "selected": False, "host": "qa@private-one"},
            {"kind": "reconnect", "generation": 2, "automatic": True,
             "selected": False, "host": "qa@private-two"},
        ]
        p5.validate(records)

    def test_extra_selected_connection_still_fails(self):
        records = self.records() + [
            {"kind": "reconnect", "generation": 7, "automatic": True,
             "selected": True, "host": "qa@private-peer"},
        ]
        with self.assertRaisesRegex(ValueError, "exactly automatic-return"):
            p5.validate(records)
