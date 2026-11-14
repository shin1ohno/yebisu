"""Transition oracle: input production is independent from expected revocation."""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)

CASES = ("E3a", "E3b", "E3c", "E3d", "E3e", "E3f-text", "E3f-key", "E3g-text", "E3g-key", "E3h", "E3i", "E3a-core")


def one(rows, kind, stage=None):
    found = [r for r in rows if r.get("kind") == kind and (stage is None or r.get("stage") == stage)]
    if len(found) != 1:
        raise ValueError(f"expected one {kind} stage={stage}: {found}")
    return found[0]


def validate(records, messages, wire, log):
    intervals = base.segments(messages)
    cores = base.core_intervals(wire)
    verdicts = {}
    for case in CASES:
        rows = [r for r in records if r.get("case") == case]
        initial = one(rows, "sh1-152-capture", 0)
        result = one(rows, "sh1-152-result", 0)
        window = [r for r in rows if initial["time"] < r["time"] <= result["time"]]
        if initial["count"] != 2:
            raise ValueError(f"retained actual items missing: {case}")
        verdict, success = "pass", case.startswith("E3g")
        if case == "E3e":
            phases = [r["phase"] for r in window if r["kind"] == "lifecycle"]
            if "Inactive" not in phases or "Background" in phases:
                verdict = "unverified: device item 10"
        if case == "E3h":
            verdict = base.check_switch(one(window, "sh1-152-switch"))
        if case.startswith("E3f"):
            external = one(window, "sh1-152-external")["result"]
            expected = "text" if case.endswith("text") else "drawing"
            kind = base.view_kind
            if kind(external["before"]) != expected or external["during"] != "UITextField" or not external["acquired"] or kind(result["responder"]) != expected:
                raise ValueError(f"external responder path did not happen: {case}: {external}, {result}")
            if result["status"] != "Revoked(ResignedOutside)":
                raise ValueError(f"external resign did not irreversibly revoke: {case}")
        if success:
            before = "text" if case.endswith("text") else "drawing"
            after = "drawing" if case.endswith("text") else "text"
            if base.view_kind(initial["responder"]) != before or base.view_kind(result["responder"]) != after or result["status"] != "Current":
                raise ValueError(f"successful internal switch revoked or did not occur: {case}")
        replay = one(window, "sh1-152-replay", 0)
        if replay["count"] != (0 if case == "E3i" else 2):
            raise ValueError(f"P4 empty-menu contract mismatch: {case}")
        if verdict == "pass":
            executions = [r for r in window if r["kind"] == "edit-menu-exec" and r["result"] == "executed"]
            if [r["action"] for r in executions] != (["Paste"] if success else []):
                raise ValueError(f"stale native command executed: {case}")
            expected_clipboard = f"sh1-152-clip-{case}-0"
            if result["clipboard"] != expected_clipboard:
                raise ValueError(f"stale Copy modified clipboard: {case}")
            if case.endswith("core"):
                if cores.get((case, 0), {}).get("events") != []:
                    raise ValueError("stale core command delivered bytes")
                if any(r["kind"] == "terminal-input" for r in window):
                    raise ValueError("stale core command reached semantic input")
            else:
                expected = [("w1:p1", {"Paste": expected_clipboard})] if success else []
                if intervals.get((case, 0), {}).get("events") != expected:
                    raise ValueError(f"stale command wire mismatch: {case}")
        if case == "E3b" and initial["generation"] == result["generation"]:
            raise ValueError("reconnect did not change generation")
        if case in ("E3a", "E3i", "E3a-core") and initial["destination"] == result["destination"]:
            raise ValueError("destination did not change")
        if case == "E3d" and not any(r["kind"] == "lifecycle" and r["phase"] == "Background" for r in window):
            raise ValueError("background case did not enter Background")
        if case == "E3i":
            ordinary = one(rows, "sh1-152-replay", 1)
            if not ordinary["ordinary"] or ordinary["count"] != 2:
                raise ValueError("unowned delegate did not preserve two ordinary suggestions")
            ordinary_end = one(rows, "sh1-152-result", 1)
            ordinary_start = one([r for r in rows if r.get("edge") == "start"], "sh1-152-marker", 1)
            if any(r["kind"] == "edit-menu-exec" for r in rows if ordinary_start["time"] < r["time"] < ordinary_end["time"]):
                raise ValueError("ordinary command acquired a menu origin")
            if intervals.get((case, 1), {}).get("events") != [("w1:p3", {"Paste": "sh1-152-clip-E3i-1"})]:
                raise ValueError("ordinary Paste did not reach B exactly once")
        fresh_stage = 2 if case == "E3i" else 1
        fresh = one(rows, "sh1-152-capture", fresh_stage)
        fresh_end = one(rows, "sh1-152-result", fresh_stage)
        if fresh["token"] == initial["token"]:
            raise ValueError("fresh presentation reused revoked identity")
        executed = [r for r in rows if r["kind"] == "edit-menu-exec" and fresh["time"] < r["time"] < fresh_end["time"]]
        if len(executed) != 1 or executed[0]["action"] != "Paste" or executed[0]["result"] != "executed":
            raise ValueError(f"fresh menu positive control missing: {case}")
        payload = f"sh1-152-clip-{case}-{fresh_stage}"
        if case.endswith("core"):
            expected = b"\x1b[200~" + payload.encode() + b"\x1b[201~"
            if base.core_segment(wire, case, fresh_stage) != expected:
                raise ValueError("fresh core menu bytes mismatch")
            if cores[(case, 0)]["destination"] == cores[(case, fresh_stage)]["destination"]:
                raise ValueError("core destination change did not occur")
            semantic = [r["variant"] for r in rows if r["kind"] == "terminal-input" and fresh["time"] < r["time"] < fresh_end["time"]]
            if any(r["terminal"] != cores[(case, fresh_stage)]["destination"] for r in rows if r["kind"] == "terminal-input" and fresh["time"] < r["time"] < fresh_end["time"]):
                raise ValueError("core semantic destination differs from wire")
            if semantic != ["Paste"]:
                raise ValueError("fresh core event was not one semantic Paste")
        else:
            destination = "w1:p3" if case in ("E3a", "E3i") else "w1:p1"
            if intervals.get((case, fresh_stage), {}).get("events") != [(destination, {"Paste": payload})]:
                raise ValueError(f"fresh wire positive mismatch: {case}")
        verdicts[case] = verdict
    for case in ("E4-text", "E4-key", "E7"):
        rows = [r for r in records if r.get("case") == case]
        one(rows, "sh1-152-result", 0)
        if any(r["kind"] == "edit-menu-exec" for r in rows):
            raise ValueError(f"ordinary input acquired a menu origin: {case}")
        events = intervals.get((case, 0), {}).get("events")
        if case == "E7":
            if "SH1-152 E7 system Edit Paste selected" in log:
                if events != [("w1:p1", {"Paste": "sh1-152-clip-E7-0"})]:
                    raise ValueError("system Edit Paste mismatch")
                verdicts[case] = "pass"
            elif "SH1-152 E7 unverified: system Edit not exposed" in log and events == []:
                verdicts[case] = "unverified: device item 9"
            else:
                raise ValueError("system Edit probe missing")
        else:
            if not events or any(pane != "w1:p1" for pane, _ in events):
                raise ValueError(f"ordinary input was lost or misrouted: {case}")
            values = [event for _, event in events]
            keys = [e["Key"] for e in values if "Key" in e]
            plain_text = values == [{"TextCommit": "x"}]
            plain_keys = len(values) == len(keys) and len(keys) in (1, 2) and all(k["code"] == {"Char": "x"} and k["modifiers"] == 0 for k in keys) and keys[0]["kind"] == "Press" and (len(keys) == 1 or keys[1]["kind"] == "Release")
            if not plain_text and not plain_keys:
                raise ValueError(f"ordinary input differs from one x: {case}: {values}")
            if any("Paste" in event for event in values):
                raise ValueError("ordinary text was an unexpected Paste")
            verdicts[case] = "pass"
    return verdicts
