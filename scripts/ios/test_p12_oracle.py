"""Synthetic oracle fixtures only; passing these is never installed-UI evidence."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p12", Path(__file__).with_name("p12-oracle.py"))
p12 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p12)


class P12OracleTests(unittest.TestCase):
    def fixture(self):
        records, ui = [], []
        state = [False, False]
        for case in p12.CASES:
            name = {"sidebar": "toggle_sidebar", "palette": "command_palette"}.get(case, "split_right")
            label = p12.LABELS.get(case, "Split Right")
            before = list(state)
            negative = case in p12.NEGATIVE
            records.append({"kind": "p12-prepare", "case": case, "sidebar": state[0], "palette": state[1],
                "pending": case in ["sidebar", "palette"], "catalog": [{"label": label, "name": name, "enabled": True}]})
            if negative:
                records.append({"kind": "p12-mutated", "case": case})
            if case in ["sidebar", "palette"]:
                i = 0 if case == "sidebar" else 1
                state[i] = not state[i]
            records.append({"kind": "p12-menu-receiver", "case": case, "name": name,
                "accepted": not negative, "pending": not negative, "before": before,
                "after": list(state), "mutated": negative})
            if negative:
                records.append({"kind": "p12-reset", "case": case, "sidebar": state[0], "palette": state[1], "mutated": True})
            ui.append({"kind": "os-menu-pick", "case": case, "owner": "com.apple.springboard", "command": label,
                "buttonLabel": label + ", Command", "enabled": True, "tapped": True})
        records += [{"kind": "p12-release", "released": True}, {"kind": "p12-finish", "enqueued": True}]
        messages = [{"ClientShellEndpointRequest": {"request": json.dumps({"method": "pane.split"})}},
                    {"ClientShellPaneInput": {"events": [{"Paste": "p12-fence"}]}}]
        shots = {"p12-finished", *[f"p12-{stage}-{case}" for stage in ["menu", "before", "after"] for case in p12.CASES]}
        return records, messages, ui, shots

    def test_joint_positive_fixture_keeps_hardware_unverified(self):
        result = p12.validate(*self.fixture())
        self.assertEqual(result["command_hold"], "unverified")

    def test_no_gui_cannot_be_replaced_by_model_or_callback(self):
        records, messages, _, shots = self.fixture()
        with self.assertRaisesRegex(ValueError, "actual OS menu"):
            p12.validate(records, messages, [], shots)

    def test_duplicate_request_fails(self):
        records, messages, ui, shots = self.fixture()
        messages.insert(0, copy.deepcopy(messages[0]))
        with self.assertRaisesRegex(ValueError, "RPC not exactly once"):
            p12.validate(records, messages, ui, shots)

    def test_key_leakage_fails(self):
        records, messages, ui, shots = self.fixture()
        messages[-1]["ClientShellPaneInput"]["events"].insert(0, {"Key": {"code": "b"}})
        with self.assertRaisesRegex(ValueError, "extra terminal"):
            p12.validate(records, messages, ui, shots)

    def test_stale_guard_acceptance_fails(self):
        records, messages, ui, shots = self.fixture()
        next(r for r in records if r.get("kind") == "p12-menu-receiver" and r["case"] == "pane")["accepted"] = True
        with self.assertRaisesRegex(ValueError, "stale consumer guard accepted"):
            p12.validate(records, messages, ui, shots)

    def test_duplicate_local_callback_fails(self):
        records, messages, ui, shots = self.fixture()
        records.append(copy.deepcopy(next(r for r in records if r.get("kind") == "p12-menu-receiver" and r["case"] == "sidebar")))
        with self.assertRaisesRegex(ValueError, "not exactly once"):
            p12.validate(records, messages, ui, shots)

    def test_missing_screenshot_fails(self):
        records, messages, ui, shots = self.fixture()
        shots.remove("p12-menu-focus")
        with self.assertRaisesRegex(ValueError, "real screenshot missing"):
            p12.validate(records, messages, ui, shots)

    def test_missing_fence_fails(self):
        records, messages, ui, shots = self.fixture()
        messages.pop()
        with self.assertRaisesRegex(ValueError, "missing FIFO fence"):
            p12.validate(records, messages, ui, shots)

    def test_unmutated_negative_fails(self):
        records, messages, ui, shots = self.fixture()
        records = [r for r in records if not (r.get("kind") == "p12-mutated" and r["case"] == "host")]
        with self.assertRaisesRegex(ValueError, "stale fault"):
            p12.validate(records, messages, ui, shots)

    def test_native_disabled_item_is_not_dispatch_proof(self):
        records, messages, ui, shots = self.fixture()
        records = [r for r in records if not (r.get("kind") == "p12-menu-receiver" and r["case"] == "focus")]
        ui[6]["tapped"] = False
        ui[6]["enabled"] = False
        self.assertEqual(p12.validate(records, messages, ui, shots)["command_hold"], "unverified")


if __name__ == "__main__":
    unittest.main()
