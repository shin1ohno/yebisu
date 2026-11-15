"""E5 oracle: two real UIKit scenes, each menu reaches only its own pane."""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)

CASES = ("E5a", "E5b", "E5c", "E5d")
FIRST, SECOND = "w1:p1", "w1:p3"
# Scene A acts first (stage 0), scene B second (stage 1): A's and B's native executions.
EXECUTED = {"E5a": (["Paste"], ["Paste"]), "E5b": (["Copy"], ["Copy"]), "E5c": ([], ["Paste"]), "E5d": ([], ["Paste"])}


def one(rows, kind, stage=None):
    found = [r for r in rows if r.get("kind") == kind and (stage is None or r.get("stage") == stage)]
    if len(found) != 1:
        raise ValueError(f"expected one {kind} stage={stage}: {found}")
    return found[0]


def validate(records, messages):
    intervals = base.segments(messages)
    verdicts = {}
    for case in CASES:
        rows = [r for r in records if r.get("case") == case]
        first, second = one(rows, "sh1-152-capture", 0), one(rows, "sh1-152-capture", 1)
        if first["count"] != 2 or second["count"] != 2:
            raise ValueError(f"actual menu items missing: {case}")
        if not first["scene"] or not second["scene"] or first["scene"] == second["scene"] or first["window"] == second["window"]:
            raise ValueError(f"two real scenes were not used: {case}: {first} {second}")
        if not one(rows, "sh1-152-second-ready")["accepted"]:
            raise ValueError(f"second scene did not move to its own pane: {case}")
        if first["token"] == second["token"]:
            raise ValueError(f"scenes shared a menu token: {case}")
        clip = [f"sh1-152-clip-{case}-{stage}" for stage in (0, 1)]
        expected = {
            0: (FIRST, [(FIRST, {"Paste": clip[0]})] if case == "E5a" else []),
            1: (SECOND, [] if case == "E5b" else [(SECOND, {"Paste": clip[1]})]),
        }
        for stage, (destination, events) in expected.items():
            interval = intervals.get((case, stage))
            if interval is None or interval["destination"] != destination or interval["events"] != events:
                raise ValueError(f"scene input reached the wrong pane: {case} stage={stage}: {interval}")
        for window, actions in zip((first["window"], second["window"]), EXECUTED[case]):
            executed = [r["action"] for r in rows if r.get("kind") == "edit-menu-exec" and r.get("window") == window and r.get("result") == "executed"]
            if executed != actions:
                raise ValueError(f"native executions differ in window {window}: {case}: {executed}")
        results = one(rows, "sh1-152-result", 0), one(rows, "sh1-152-result", 1)
        if case == "E5b":
            selections = [one(rows, "sh1-152-selection", stage)["text"] for stage in (0, 1)]
            if selections != ["AAA-152", "BBB-152"] or [r["clipboard"] for r in results] != selections:
                raise ValueError(f"Copy did not use its own scene's selection: {selections} {results}")
        if case == "E5c":
            if one(rows, "sh1-152-selection", 0)["text"] != "AAA-152":
                raise ValueError("scene A selection for the stale Copy was not made")
            if len(one(rows, "sh1-152-cross-scene-replay")["sent"]) != 2:
                raise ValueError("scene A's retained items were not replayed in scene B")
            if results[1]["clipboard"] != clip[1]:
                raise ValueError("scene A's stale Copy changed the clipboard")
        if case == "E5d" and not one(rows, "sh1-152-close-other")["closed"]:
            raise ValueError("scene A was not closed")
        verdicts[case] = "pass"
    return verdicts
