"""Synthetic oracle tests only. These never claim native execution."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p14", Path(__file__).with_name("p14-check.py"))
p14 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p14)


def fixture():
    commands = ["p14-live", "p14-quiet", "p14-invalid", "p14-disconnect"]
    records = [dict(kind="p14-step", step=i, sent=i < 5, generation_before=1, generation_after=1) for i in range(1, 6)]
    base = dict(kind="p14-paint", step=1, toast=None, bounds=[1, 1, 200, 80], boot="boot-v1", host="qa@private-peer", active=True, connected=True, pane="w1:p1", authorization="NotDetermined")
    records += [dict(base, toast="Agent needs attention"), dict(base, pane="w1:p3")]
    records += [dict(base, step=step, status=status, active=step < 4, connected=step < 4) for step, status in [(2, "QuietHours"), (3, "InvalidTarget"), (4, "Disconnected"), (5, "Stale")]]
    messages = [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Paste": command}]}} for command in commands]
    messages += [{"ClientShellEndpointRequest": {"boot_id": "boot-v1", "request": json.dumps({"method": "pane.focus", "params": {"pane_id": "w1:p3"}})}}]
    sent = [{"command": c, "message": {"SemanticNotification": {"pane_id": pane, "sound": None}}} for c, pane in zip(commands, ["w1:p3", "w1:p1", "missing-pane", "w1:p1"])]
    return records, messages, sent, set(p14.NAMES)


class P14Oracle(unittest.TestCase):
    def test_fixture_preserves_unverified_os_and_sound(self):
        result = p14.validate(*fixture())
        self.assertEqual(result["os_banner"], "UNVERIFIED")
        self.assertEqual(result["sound"], "UNVERIFIED")
        self.assertEqual(result["actual_authorization_query"], ["NotDetermined"])

    def test_no_show_negative_detects_toast(self):
        data = fixture()
        data[0][-1]["toast"] = "Agent needs attention"
        with self.assertRaisesRegex(ValueError, "negative phase"):
            p14.validate(*data)

    def test_missing_wire_rejected(self):
        data = fixture()
        data[2].pop()
        with self.assertRaisesRegex(ValueError, "sending peer"):
            p14.validate(*data)

    def test_duplicate_tap_rejected(self):
        data = fixture()
        data[1].append(copy.deepcopy(data[1][-1]))
        with self.assertRaisesRegex(ValueError, "exactly once"):
            p14.validate(*data)

    def test_unknown_authorization_is_not_query_proof(self):
        data = fixture()
        for r in data[0]: r["authorization"] = "Unknown"
        with self.assertRaisesRegex(ValueError, "query not observed"):
            p14.validate(*data)

    def test_missing_screenshot_rejected(self):
        data = fixture()
        data[3].remove("p14-toast")
        with self.assertRaisesRegex(ValueError, "screenshot"):
            p14.validate(*data)

    def test_old_context_and_reconnect_rejected(self):
        for field in ["host", "boot"]:
            data = fixture()
            data[0][5][field] = "old"
            with self.assertRaisesRegex(ValueError, "coherent actual toast"):
                p14.validate(*data)
        data = fixture()
        data[0][4]["generation_after"] = 2
        with self.assertRaisesRegex(ValueError, "reconnected"):
            p14.validate(*data)
