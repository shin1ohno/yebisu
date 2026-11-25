"""rename-check.py: the peer log splits by marker, rename requests and pane input are counted
per step, and each verdict fails for the reason it guards."""
import importlib.util
import json
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("rename_check", HERE / "rename-check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


def rpc(method, **params):
    return {"ClientShellEndpointRequest": {"request": json.dumps({"method": method, "params": params})}}


def text(value, pane="w1:p1"):
    return {"ClientShellPaneInput": {"pane_id": pane, "events": [{"TextCommit": value}]}}


def field(time, open_=True, first=True, language="en-US", marked=False):
    return {"kind": "rename-state", "time": time, "open": open_,
            "sdk": {"field_first_responder": first, "gpui_text_first_responder": False,
                    "gpui_drawing_first_responder": False, "primary_language": language, "marked": marked}}


TIMES = [("R1", 10.0), ("R2", 20.0), ("R2b", 25.0), ("R3", 30.0), ("R4", 40.0), ("R5", 50.0), ("end", 60.0)]


def good_peer():
    return [rpc("workspace.rename", workspace_id="w1", label="ship"), text("z"),
            rpc("tab.focus", tab_id="w1:t1"), rpc("tab.rename", tab_id="w1:t1", label="build"), text("y"),
            rpc("workspace.rename", workspace_id="w1", label="space2"), text("u"),
            text("x"), text("w"),
            rpc("workspace.rename", workspace_id="w1", label="ぷろ"),
            {"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Key": {"kind": "Press", "code": "Enter"}}]}},
            text("v")]


def good_records():
    return [field(t + 1, language="ja-JP" if name == "R5" else "en-US") for name, t in TIMES[:-1]]


class Verdicts(unittest.TestCase):
    def verdicts(self, peer, records):
        windows = check.by_step(records, TIMES)
        _, steps = check.split_peer(peer)
        return check.verdicts(windows, steps)

    def test_a_clean_run_passes_every_step(self):
        result = self.verdicts(good_peer(), good_records())
        self.assertEqual({name: ok for name, (ok, _) in result.items()}, {name: True for name in check.STEPS})
        self.assertIn("ja-JP", result["R5"][1])

    def test_a_second_request_fails_r1(self):
        peer = good_peer()
        peer.insert(1, rpc("workspace.rename", workspace_id="w1", label="ship"))
        self.assertFalse(self.verdicts(peer, good_records())["R1"][0])

    def test_a_key_reaching_the_terminal_fails_r4(self):
        peer = good_peer()
        peer.insert(peer.index(text("w")), text("q"))
        ok, reason = self.verdicts(peer, good_records())["R4"]
        self.assertFalse(ok)
        self.assertIn("'q'", reason)

    def test_a_cancelled_field_that_sent_fails_r3(self):
        peer = good_peer()
        peer.insert(peer.index(text("x")), rpc("workspace.rename", workspace_id="w1", label="never"))
        self.assertFalse(self.verdicts(peer, good_records())["R3"][0])

    def test_a_field_never_first_responder_fails(self):
        records = [field(t + 1, first=False) for _, t in TIMES[:-1]]
        result = self.verdicts(good_peer(), records)
        self.assertTrue(all(not ok for ok, _ in result.values()))

    def test_the_tab_request_must_carry_the_name(self):
        peer = [rpc("tab.rename", tab_id="w1:t1", label="other") if r == rpc("tab.rename", tab_id="w1:t1", label="build")
                else r for r in good_peer()]
        self.assertFalse(self.verdicts(peer, good_records())["R2"][0])


    def test_a_second_move_fails_r2b(self):
        peer = good_peer()
        at = peer.index(text("u"))
        peer[at:at] = [rpc("workspace.focus", workspace_id="w1"), rpc("workspace.focus", workspace_id="w1")]
        self.assertFalse(self.verdicts(peer, good_records())["R2b"][0])

    def test_r5_takes_only_the_trailing_return(self):
        peer = good_peer()
        peer.insert(peer.index(text("v")), text("q"))
        self.assertFalse(self.verdicts(peer, good_records())["R5"][0])


    def test_a_cancelled_double_tap_is_unverified_not_passed(self):
        peer = [r for r in good_peer() if r != rpc("tab.rename", tab_id="w1:t1", label="build")]
        records = [r for r in good_records() if not (20.0 <= r["time"] < 25.0)]
        windows = check.by_step(records, TIMES)
        _, steps = check.split_peer(peer)
        ok, reason = check.verdicts(windows, steps, {"R2": 1})["R2"]
        self.assertIsNone(ok)
        self.assertIn("D4", reason)
        self.assertFalse(check.verdicts(windows, steps, {})["R2"][0], "without a cancel it fails")


class Parsing(unittest.TestCase):
    def test_paste_and_keys_are_inputs(self):
        record = {"ClientShellPaneInput": {"pane_id": "p", "events": [
            {"Paste": "renamepasteword"}, {"Key": {"kind": "Press", "code": "b"}}, {"Key": {"kind": "Release"}}]}}
        self.assertEqual(check.peer_events(record), [("paste", "renamepasteword"), ("key", "b")])

    def test_untrusted_shapes_are_ignored(self):
        for record in (None, [], {"a": 1, "b": 2}, {"ClientShellEndpointRequest": {"request": "not json"}},
                       {"ClientShellEndpointRequest": {"request": "[1]"}}):
            self.assertEqual(check.peer_events(record), [])

    def test_steps_come_from_the_test_output(self):
        stdout = "noise\nRENAME_STEP R1 10.5\nRENAME_STEP bad x\nRENAME_STEP R2 11\n"
        self.assertEqual(check.step_times(stdout), [("R1", 10.5), ("R2", 11.0)])


if __name__ == "__main__":
    unittest.main()
