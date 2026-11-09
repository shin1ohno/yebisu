import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p7", Path(__file__).with_name("p7-check.py"))
p7 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p7)

class P7OracleTests(unittest.TestCase):
    def records(self):
        states = [("w1:t1", "w1:p1", 1, 2), ("w1:t3", "w1:p13", 2, 1),
                  ("w1:t3", "w1:p14", 2, 2), ("w1:t1", "w1:p1", 2, 2)]
        return [dict(kind="terminal", boot="boot-v1", tab=t, pane=p, input_pane=p,
                     tab_count=nt, pane_count=np) for t, p, nt, np in states]

    def messages(self):
        return [{"ClientShellEndpointRequest": {"request": json.dumps(dict(method=m, params=p))}}
                for m, p in [("tab.create", {"workspace_id": "w1"}),
                             ("pane.split", {"direction": "right"}),
                             ("tab.focus", {"tab_id": "w1:t1"})]]

    def test_positive(self):
        p7.validate(self.records(), self.messages())

    def test_ack_without_state_change_fails(self):
        with self.assertRaisesRegex(ValueError, "missing coherent"):
            p7.validate([self.records()[0]], self.messages())

    def test_old_boot_or_wrong_input_destination_fails(self):
        for key, value in [("boot", "old"), ("input_pane", "wrong"), ("tab_count", 1), ("pane_count", 1)]:
            records = copy.deepcopy(self.records())
            records[2][key] = value
            with self.assertRaisesRegex(ValueError, "missing coherent"):
                p7.validate(records, self.messages())

    def test_wrong_api_or_synthetic_keys_fail(self):
        with self.assertRaisesRegex(ValueError, "incorrect API"):
            p7.validate(self.records(), self.messages()[:-1])
        with self.assertRaisesRegex(ValueError, "synthesized"):
            p7.validate(self.records(), self.messages()+[{"ClientShellPaneInput": {}}])


class R6OracleTests(unittest.TestCase):
    def records(self):
        records = [dict(kind="terminal", boot="boot-v1", tab=t, pane=p, input_pane=p)
                   for t, p in [("w1:t3", "w1:p13"), ("w1:t1", "w1:p1"), ("w1:t3", "w1:p13")]]
        return records + [dict(kind="r6-key", key=k, palette=False, consumed=0) for k in ["}", "{", "escape"]]

    def messages(self):
        return [{"ClientShellEndpointRequest": {"request": json.dumps(dict(method=m, params=p))}}
                for m, p in [("tab.create", {}), ("tab.focus", {"tab_id": "w1:t1"}),
                             ("tab.focus", {"tab_id": "w1:t3"})]]

    def test_positive(self):
        p7.validate_r6(self.records(), self.messages())

    def test_repeat_or_missing_api_fails(self):
        for messages in [self.messages()[:-1], self.messages() + self.messages()[-1:]]:
            with self.assertRaisesRegex(ValueError, "API"):
                p7.validate_r6(self.records(), messages)

    def test_escaped_terminal_input_fails(self):
        with self.assertRaisesRegex(ValueError, "leaked"):
            p7.validate_r6(self.records(), self.messages() + [{"ClientShellPaneInput": {}}])

    def test_palette_release_and_wrong_destination_fail(self):
        for index, key, value in [(5, "palette", True), (5, "consumed", 1),
                                  (1, "input_pane", "wrong"), (1, "boot", "old")]:
            records = self.records()
            records[index][key] = value
            with self.assertRaises(ValueError):
                p7.validate_r6(records, self.messages())
