import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("transitions", Path(__file__).with_name("sh1_152_transitions.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def fixture():
    records, messages, wire = [], [], []

    def record(case, kind, time, **extra):
        row = {"case": case, "kind": kind, "time": time, **extra}
        records.append(row)
        return row

    def interval(case, stage, destination, events):
        values = [{"Paste": f"sh1-152-start-{case}-{stage}"}, *events, {"Paste": f"sh1-152-end-{case}-{stage}"}]
        if case.endswith("core"):
            wire.extend({"terminal": destination, "bytes": list(b"\x1b[200~" + event["Paste"].encode() + b"\x1b[201~")} for event in values)
        else:
            messages.extend({"ClientShellPaneInput": {"pane_id": destination, "events": [event]}} for event in values)

    for case in oracle.CASES:
        success = case.startswith("E3g")
        before = "GPUITextInputView" if case.endswith("text") else "HerdrUIKitGPUIView"
        after = ("HerdrUIKitGPUIView" if case.endswith("text") else "GPUITextInputView") if success else before
        record(case, "sh1-152-capture", 1, stage=0, count=2, token=1, responder=before, generation=1, destination="A")
        status = "Current" if success else ("Revoked(ResignedOutside)" if case.startswith("E3f") else "Revoked(Dismissed)")
        record(case, "sh1-152-result", 4, stage=0, clipboard=f"sh1-152-clip-{case}-0", status=status, responder=after, generation=2, destination="B")
        record(case, "sh1-152-replay", 3, stage=0, count=0 if case == "E3i" else 2, ordinary=False)
        if case in ("E3d", "E3e"):
            record(case, "lifecycle", 2, phase="Background" if case == "E3d" else "Inactive")
        if case == "E3h":
            record(case, "sh1-152-switch", 2, dropped=0, status="Revoked(SwitchLeftNoResponder)", samples=[{"restart_reached": True, "acquire_returns": [False], "drawing_owned": True, "text_owned": False}])
        if case.startswith("E3f"):
            record(case, "sh1-152-external", 2, result={"before": before, "during": "UITextField", "acquired": True})
        events = []
        if success:
            events = [{"Paste": f"sh1-152-clip-{case}-0"}]
            record(case, "edit-menu-exec", 2, action="Paste", result="executed")
        interval(case, 0, "t1" if case.endswith("core") else "w1:p1", events)
        for stage in range(1, 3 if case == "E3i" else 2):
            start = 10 + stage * 10
            record(case, "sh1-152-marker", start-1, stage=stage, edge="start")
            record(case, "sh1-152-capture", start, stage=stage, token=stage+1)
            record(case, "sh1-152-result", start+3, stage=stage)
            if case == "E3i" and stage == 1:
                record(case, "sh1-152-replay", start+1, stage=stage, ordinary=True, count=2)
            else:
                record(case, "edit-menu-exec", start+1, action="Paste", result="executed")
            destination = "t2" if case.endswith("core") else "w1:p3" if case in ("E3a", "E3i") else "w1:p1"
            interval(case, stage, destination, [{"Paste": f"sh1-152-clip-{case}-{stage}"}])
            if case.endswith("core"):
                record(case, "terminal-input", start+1.5, variant="Paste", terminal="t2")
    for case in ("E4-text", "E4-key", "E7"):
        record(case, "sh1-152-result", 3, stage=0)
        interval(case, 0, "w1:p1", [] if case == "E7" else [{"TextCommit": "x"}])
    return records, messages, wire, "SH1-152 E7 unverified: system Edit not exposed"


class TransitionsTests(unittest.TestCase):
    def test_positive_and_independent_unverified_inputs(self):
        data = fixture()
        result = oracle.validate(*data)
        self.assertEqual(result["E3h"], "pass")
        self.assertEqual(result["E3i"], "pass")
        self.assertIn("unverified", result["E7"])
        switch = next(r for r in data[0] if r["kind"] == "sh1-152-switch")
        switch["samples"][0]["acquire_returns"] = [True]
        switch["status"] = "Current"
        self.assertIn("unverified", oracle.validate(*data)["E3h"])

    def test_negatives_fail_for_the_intended_contract(self):
        for mutation, expected in [
            ("current", "produced switch failure"), ("external", "irreversibly revoke"),
            ("delegate", "empty-menu contract"), ("ordinary", "ordinary suggestions"),
            ("reused", "reused revoked"), ("core-key", "semantic Paste"),
            ("core-destination", "semantic destination"), ("stale-exec", "stale native"),
            ("reconnect", "reconnect did not change"), ("destination", "destination did not change"),
            ("extra-key", "differs from one x"),
        ]:
            records, messages, wire, log = copy.deepcopy(fixture())
            find = lambda case, kind, stage=None: next(r for r in records if r["case"] == case and r["kind"] == kind and (stage is None or r.get("stage") == stage))
            if mutation == "current": find("E3h", "sh1-152-switch")["status"] = "Current"
            elif mutation == "external": find("E3f-text", "sh1-152-result", 0)["status"] = "Current"
            elif mutation == "delegate": find("E3i", "sh1-152-replay", 0)["count"] = 2
            elif mutation == "ordinary": find("E3i", "sh1-152-replay", 1)["count"] = 0
            elif mutation == "reused": find("E3a", "sh1-152-capture", 1)["token"] = 1
            elif mutation == "core-key": find("E3a-core", "terminal-input")["variant"] = "Key"
            elif mutation == "core-destination": find("E3a-core", "terminal-input")["terminal"] = "t1"
            elif mutation == "stale-exec": records.append({"case":"E3a", "kind":"edit-menu-exec", "time":2, "action":"Paste", "result":"executed"})
            elif mutation == "reconnect": find("E3b", "sh1-152-result", 0)["generation"] = 1
            elif mutation == "destination": find("E3a", "sh1-152-result", 0)["destination"] = "A"
            else:
                index = next(i for i, m in enumerate(messages) if m["ClientShellPaneInput"]["events"] == [{"Paste":"sh1-152-end-E4-text-0"}])
                messages.insert(index, {"ClientShellPaneInput":{"pane_id":"w1:p1", "events":[{"Key":{"code":{"Char":"z"},"modifiers":0,"kind":"Press"}}]}})
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, expected):
                oracle.validate(records, messages, wire, log)


if __name__ == "__main__":
    unittest.main()
