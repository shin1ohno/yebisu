import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("scenes", Path(__file__).with_name("sh1_152_scenes.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def fixture():
    records, messages = [], []
    for case in oracle.CASES:
        def record(kind, time, **extra):
            records.append({"case": case, "kind": kind, "time": time, **extra})
        for stage, (window, pane) in enumerate([(0, "w1:p1"), (1, "w1:p3")]):
            clip = f"sh1-152-clip-{case}-{stage}"
            text = "AAA-152" if stage == 0 else "BBB-152"
            record("sh1-152-capture", 10 * stage + 1, stage=stage, window=window, count=2, scene=f"scene-{stage}", token=stage + 1)
            if case in ("E5b", "E5c") and (stage == 0 or case == "E5b"):
                record("sh1-152-selection", 10 * stage, stage=stage, window=window, text=text)
            actions = oracle.EXECUTED[case][stage]
            for action in actions:
                record("edit-menu-exec", 10 * stage + 2, window=window, action=action, result="executed")
            clipboard = text if case == "E5b" else clip
            record("sh1-152-result", 10 * stage + 3, stage=stage, window=window, clipboard=clipboard)
            events = [{"Paste": clip}] if "Paste" in actions else []
            values = [{"Paste": f"sh1-152-start-{case}-{stage}"}, *events, {"Paste": f"sh1-152-end-{case}-{stage}"}]
            messages.extend({"ClientShellPaneInput": {"pane_id": pane, "events": [v]}} for v in values)
        record("sh1-152-second-ready", 5, window=1, accepted=True)
        if case == "E5c":
            record("sh1-152-cross-scene-replay", 11, window=1, sent=[True, True])
        if case == "E5d":
            record("sh1-152-close-other", 6, window=1, closed=True)
    return records, messages


class ScenesTests(unittest.TestCase):
    def test_positive(self):
        self.assertEqual(set(oracle.validate(*fixture()).values()), {"pass"})

    def test_negatives(self):
        for mutation, expected in [
            ("same-scene", "two real scenes"), ("wrong-pane", "wrong pane"), ("stale-paste", "wrong pane"),
            ("stale-exec", "native executions"), ("copy-other", "own scene"), ("stale-copy", "stale Copy"),
            ("not-closed", "not closed"), ("no-replay", "retained items"), ("shared-token", "shared a menu token"),
        ]:
            records, messages = copy.deepcopy(fixture())
            find = lambda case, kind, stage=None: next(r for r in records if r["case"] == case and r["kind"] == kind and (stage is None or r.get("stage") == stage))
            if mutation == "same-scene": find("E5a", "sh1-152-capture", 1)["scene"] = "scene-0"
            elif mutation == "wrong-pane":
                m = next(m for m in messages if m["ClientShellPaneInput"]["events"] == [{"Paste": "sh1-152-clip-E5a-1"}])
                m["ClientShellPaneInput"]["pane_id"] = "w1:p1"
            elif mutation == "stale-paste":
                i = next(i for i, m in enumerate(messages) if m["ClientShellPaneInput"]["events"] == [{"Paste": "sh1-152-end-E5c-1"}])
                messages.insert(i, {"ClientShellPaneInput": {"pane_id": "w1:p3", "events": [{"Paste": "sh1-152-clip-E5c-1"}]}})
            elif mutation == "stale-exec": records.append({"case": "E5c", "kind": "edit-menu-exec", "time": 12, "window": 0, "action": "Copy", "result": "executed"})
            elif mutation == "copy-other": find("E5b", "sh1-152-result", 1)["clipboard"] = "AAA-152"
            elif mutation == "stale-copy": find("E5c", "sh1-152-result", 1)["clipboard"] = "AAA-152"
            elif mutation == "not-closed": find("E5d", "sh1-152-close-other")["closed"] = False
            elif mutation == "no-replay": find("E5c", "sh1-152-cross-scene-replay")["sent"] = []
            else: find("E5b", "sh1-152-capture", 1)["token"] = 1
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, expected):
                oracle.validate(records, messages)


if __name__ == "__main__":
    unittest.main()
