"""Every failure branch of the P11 oracles must be reachable by a test.

A branch that no test reaches can be deleted without anyone noticing, and the native
collector would then report observed controls for a broken product. Each case below
names the message of the branch it forces, so replacing that branch with `pass` fails
this file instead of surviving silently."""
import copy
import importlib.util
from pathlib import Path
import unittest

HERE = Path(__file__).parent


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


oracle = load("oracle_failures", "p11-observer-oracle.py")
check = load("p11_failures", "p11-check.py")
observer_fixture = load("observer_fixture", "test_p11_observer_oracle.py").fixture
check_fixture = load("check_fixture", "test_p11_check.py").P11Oracle().fixture


class ProbeChecks(unittest.TestCase):
    def test_probe_must_have_started_scheduled_and_blocked(self):
        for field in ("scheduled", "action_blocked"):
            records, messages, disk = check_fixture()
            records[0][field] = False
            with self.assertRaisesRegex(ValueError, "modal callback probe"):
                check.validate(records, messages, disk)
        records, messages, disk = check_fixture()
        records[1]["modal"] = False
        with self.assertRaisesRegex(ValueError, "modal callback probe"):
            check.validate(records, messages, disk)

    def test_wrong_receiving_pane(self):
        records, messages, disk = check_fixture()
        messages[0]["ClientShellPaneInput"]["pane_id"] = "w1:p2"
        with self.assertRaisesRegex(ValueError, "wrong receiving pane"):
            check.validate(records, messages, disk)

    def test_font_must_reach_the_painter_and_the_disk(self):
        records, messages, disk = check_fixture()
        with self.assertRaisesRegex(ValueError, "durable state mismatch"):
            check.validate(records, messages, {"terminal_font_size": 14})
        records = [r for r in records if r.get("kind") != "terminal"]
        with self.assertRaisesRegex(ValueError, "durable state mismatch"):
            check.validate(records, messages, disk)

    def test_key_material_never_reaches_the_observation(self):
        records, messages, disk = check_fixture()
        records.append({"kind": "settings", "key_line": "ssh-ed25519 AAAA"})
        with self.assertRaisesRegex(ValueError, "key leaked"):
            check.validate(records, messages, disk)

    def test_receiving_wire_leak(self):
        records, messages, disk = check_fixture()
        messages[0]["ClientShellPaneInput"]["events"].append({"TextCommit": "leak"})
        with self.assertRaisesRegex(ValueError, "wire leak"):
            check.validate(records, messages, disk)


def settings_fixture():
    records = [
        {"kind": "terminal", "font_size": 15, "cell_size": [18, 43]},
        {"kind": "settings-loaded", "font_size": None},
        {"kind": "settings-loaded", "font_size": 15},
    ]
    messages = [{"ClientShellResize": {"cell_width_px": 18, "cell_height_px": 43}}]
    return records, messages, {"terminal_font_size": 15}


class SettingsOracle(unittest.TestCase):
    def test_valid_shape(self):
        oracle.settings(*settings_fixture())

    def test_font_consumer_disk_and_relaunch_are_all_required(self):
        records, messages, disk = settings_fixture()
        with self.assertRaisesRegex(ValueError, "disk/relaunch"):
            oracle.settings(records, messages, {"terminal_font_size": 14})
        with self.assertRaisesRegex(ValueError, "disk/relaunch"):
            oracle.settings([r for r in records if r["kind"] != "terminal"], messages, disk)
        stale = copy.deepcopy(records)
        stale[-1]["font_size"] = 14
        with self.assertRaisesRegex(ValueError, "disk/relaunch"):
            oracle.settings(stale, messages, disk)
        with self.assertRaisesRegex(ValueError, "disk/relaunch"):
            oracle.settings(records[:2], messages, disk)

    def test_receiver_must_see_the_new_cell_size(self):
        records, messages, disk = settings_fixture()
        messages[0]["ClientShellResize"]["cell_height_px"] = 40
        with self.assertRaisesRegex(ValueError, "font resize"):
            oracle.settings(records, messages, disk)
        with self.assertRaisesRegex(ValueError, "font resize"):
            oracle.settings(records, [], disk)

    def test_key_line_never_appears(self):
        records, messages, disk = settings_fixture()
        records.append({"kind": "x", "key_line": "ssh-ed25519 AAAA"})
        with self.assertRaisesRegex(ValueError, "key observation"):
            oracle.settings(records, messages, disk)


def positive_fixture():
    records, events = [], []
    for i in range(24):
        records.append({"kind": "native-start", "case": i,
                        "action": ["paste:", "copy:", "cut:", "selectAll:", "physical-v", "physical-c"][i % 6],
                        "command_control": i // 6 >= 2, "terminal_priority": i // 6 % 2 == 1})
        records.append({"kind": "native-result", "case": i, "clipboard_matches": True,
                        "workers": 0, "preparing": False})
        events.append({"Paste": f"native-start-{i}"})
        if i // 6 == 3 and i % 6 in (4, 5):
            code = "v" if i % 6 == 4 else "c"
            for edge in ("Press", "Release"):
                events.append({"Key": {"code": {"Char": code}, "kind": edge, "modifiers": 2}})
        elif i % 6 in (0, 4):
            events.append({"Paste": f"native-text-{i}"})
        events.append({"Paste": f"native-end-{i}"})
        if i % 6 >= 4:
            records.append({"kind": "p11-physical-control", "case": i, "origin_none": True})
    return records, [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": events}}]


class PositiveOracle(unittest.TestCase):
    def test_valid_shape(self):
        oracle.positive(*positive_fixture())

    def test_all_24_cases_are_required(self):
        records, messages = positive_fixture()
        records.remove(next(r for r in records if r["kind"] == "native-result"))
        with self.assertRaisesRegex(ValueError, "24-case"):
            oracle.positive(records, messages)

    def test_wrong_settings_for_a_case(self):
        records, messages = positive_fixture()
        next(r for r in records if r["kind"] == "native-start")["action"] = "copy:"
        with self.assertRaisesRegex(ValueError, "positive settings"):
            oracle.positive(records, messages)

    def test_async_and_clipboard_completion(self):
        for field, value in (("clipboard_matches", False), ("workers", 1), ("preparing", True)):
            records, messages = positive_fixture()
            next(r for r in records if r["kind"] == "native-result")[field] = value
            with self.assertRaisesRegex(ValueError, "completion missing"):
                oracle.positive(records, messages)

    def test_fifo_content_count_and_physical_wire(self):
        records, messages = positive_fixture()
        messages[0]["ClientShellPaneInput"]["events"][1] = {"Paste": "native-text-WRONG"}
        with self.assertRaisesRegex(ValueError, "FIFO mismatch"):
            oracle.positive(records, messages)
        records, messages = positive_fixture()
        messages[0]["ClientShellPaneInput"]["events"].append({"TextCommit": "leak"})
        with self.assertRaisesRegex(ValueError, "FIFO count"):
            oracle.positive(records, messages)
        records, messages = positive_fixture()
        key = next(e for e in messages[0]["ClientShellPaneInput"]["events"] if "Key" in e)
        key["Key"]["modifiers"] = 0
        with self.assertRaisesRegex(ValueError, "physical wire"):
            oracle.positive(records, messages)

    def test_physical_cases_must_have_no_origin(self):
        records, messages = positive_fixture()
        next(r for r in records if r["kind"] == "p11-physical-control")["origin_none"] = False
        with self.assertRaisesRegex(ValueError, "physical origin"):
            oracle.positive(records, messages)
        records, messages = positive_fixture()
        records.remove(next(r for r in records if r["kind"] == "p11-physical-control"))
        with self.assertRaisesRegex(ValueError, "physical origin"):
            oracle.positive(records, messages)


class ValidateFailures(unittest.TestCase):
    def test_status_order(self):
        records, _ = observer_fixture()
        drain = [i for i, r in enumerate(records) if r["kind"] == "p11-drain"][-1]
        status = drain + 1
        records[drain], records[status] = records[status], records[drain]
        with self.assertRaisesRegex(ValueError, "status order"):
            oracle.validate(records, modal=True)

    def test_orphan_consumer_receipt(self):
        records, _ = observer_fixture()
        entry = next(r for r in records if r.get("event") == "Entry")
        orphan = {"kind": "p11-consumer", "route": "sheet-modal", "edge": "down",
                  "key": {"arm": copy.deepcopy(entry["key"]["arm"]), "invocation": 999}}
        records.insert(records.index(entry) + 1, orphan)
        with self.assertRaisesRegex(ValueError, "orphan consumer"):
            oracle.validate(records, modal=True)

    def test_phase_policy_action_and_origin(self):
        for field, value in (("policy", [True] * 4), ("action", "Cut"), ("responder", "Drawing")):
            records, _ = observer_fixture()
            next(r for r in records if r["kind"] == "p11-arm")[field] = value
            with self.assertRaisesRegex(ValueError, "phase policy"):
                oracle.validate(records, modal=True)

    def test_missing_arm_manifest(self):
        records, _ = observer_fixture()
        records.remove(next(r for r in records if r["kind"] == "p11-arm"))
        with self.assertRaisesRegex(ValueError, "missing arm manifest"):
            oracle.validate(records, modal=True)

    def test_disabled_observer_emits_nothing(self):
        records, _ = observer_fixture(True)
        sdk = next(r for r in observer_fixture()[0] if r["kind"] == "p11-sdk")
        records.insert(1, sdk)
        with self.assertRaisesRegex(ValueError, "disabled observer emitted"):
            oracle.validate(records, modal=True, disabled=True, allow_disabled=True)

    def test_modal_wire_carries_only_the_fifo_markers(self):
        _, messages = observer_fixture()
        self.assertEqual(len(oracle.fifo(messages, True)), 48)
        messages.append({"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Paste": "leak"}]}})
        with self.assertRaisesRegex(ValueError, "modal wire leak"):
            oracle.fifo(messages, True)

    def test_positive_control_requires_the_non_modal_arm_layout(self):
        records, _ = observer_fixture()
        with self.assertRaisesRegex(ValueError, "missing arm manifest"):
            oracle.validate(records, modal=False)


if __name__ == "__main__":
    unittest.main()
