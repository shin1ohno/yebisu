"""Exact event boundaries, destination identity and independent failure inputs."""
import re

MARKER = re.compile(r"^sh1-152-(start|end)-(.+)-(\d+)$")
BASIC_CASES = ("E1-text", "E1-key", "E2-text", "E2-key", "E1-core", "E2-core")


def segments(messages):
    current, result = None, {}
    for message in messages:
        payload = message.get("ClientShellPaneInput")
        if payload is None:
            continue
        for event in payload["events"]:
            marker = MARKER.fullmatch(event.get("Paste", ""))
            if marker:
                edge, case, stage = marker.groups()
                key = (case, int(stage))
                if edge == "start":
                    if current is not None or key in result:
                        raise ValueError("duplicate or overlapping start")
                    current = key
                    result[key] = {"destination": payload["pane_id"], "events": []}
                elif current != key:
                    raise ValueError("unmatched end")
                else:
                    current = None
            elif current is None:
                raise ValueError("input outside marked interval")
            else:
                result[current]["events"].append((payload["pane_id"], event))
    if current is not None:
        raise ValueError("unfinished interval")
    return result


VISIBLE = re.compile(r"^SH1-152 VISIBLE (\S+) selected=(true|false) labels=(.*)$", re.M)
SELECTED_AT_MENU = {"E2-text", "E2-key", "E2-core", "E3c", "E3f-text", "E3f-key", "E5b", "E5c"}


def visible_menus(log, cases):
    """Owner rule: only usable items are shown (Paste alone, or Copy and Paste with a selection)."""
    seen = {}
    for case, selected, labels in VISIBLE.findall(log):
        selected = selected == "true"
        expected = ["Copy", "Paste"] if selected else ["Paste"]
        if sorted(labels.split(",")) != expected:
            raise ValueError(f"visible menu violates usable-items rule: {case} selected={selected} {labels}")
        seen.setdefault(case, []).append(selected)
    for case in cases:
        if case not in seen:
            raise ValueError(f"visible menu not recorded: {case}")
        if seen[case][0] != (case in SELECTED_AT_MENU):
            raise ValueError(f"first menu selection state differs from the case: {case}")
    return "SH1-152 visible menus PASS: " + ", ".join(f"{c}={len(v)}" for c, v in sorted(seen.items()))


def view_kind(name):
    """UIKit reports the Rust-declared class with its module path and version."""
    if name and "GPUITextInputView" in name:
        return "text"
    if name and "HerdrUIKitGPUIView" in name:
        return "drawing"
    return name


def check_switch(record):
    if record["dropped"]:
        raise ValueError("switch observations dropped")
    samples = [s for s in record["samples"] if s["restart_reached"]]
    produced = any(False in s["acquire_returns"] or not (s["drawing_owned"] or s["text_owned"]) for s in samples)
    if not produced:
        return "unverified: device item 12"
    if record["status"] != "Revoked(SwitchLeftNoResponder)":
        raise ValueError("produced switch failure retained usable token")
    return "pass"


def core_intervals(wire):
    active, intervals = None, {}
    for row in wire:
        payload = bytes(row["bytes"])
        marker = None
        if payload.startswith(b"\x1b[200~") and payload.endswith(b"\x1b[201~"):
            marker = MARKER.fullmatch(payload[6:-6].decode("utf-8"))
        if marker:
            edge, case, stage = marker.groups()
            key = (case, int(stage))
            if edge == "start":
                if active is not None or key in intervals:
                    raise ValueError("duplicate or overlapping core start")
                active = key
                intervals[key] = {"destination": row["terminal"], "events": []}
            elif active != key:
                raise ValueError("unmatched core end")
            else:
                active = None
        elif active is None:
            raise ValueError("core input outside interval")
        else:
            intervals[active]["events"].append((row["terminal"], payload))
    if active is not None:
        raise ValueError("unfinished core interval")
    return intervals


def core_segment(wire, case, stage=0):
    interval = core_intervals(wire).get((case, stage))
    if interval is None:
        raise ValueError("core interval missing")
    if any(destination != interval["destination"] for destination, _ in interval["events"]):
        raise ValueError("core input reached another terminal")
    return b"".join(payload for _, payload in interval["events"])


def validate_basic(records, messages, wire):
    intervals = segments(messages)
    for case in BASIC_CASES:
        rows = [r for r in records if r.get("case") == case]
        results = [r for r in rows if r.get("kind") == "sh1-152-result"]
        captures = [r for r in rows if r.get("kind") == "sh1-152-capture"]
        executions = [r for r in rows if r.get("kind") == "edit-menu-exec"]
        if len(results) != 1 or len(captures) != 1 or captures[0]["count"] != 2:
            raise ValueError(f"missing/duplicated actual presentation: {case}")
        copy = case.startswith("E2")
        expected = "日本語" if copy else f"sh1-152-clip-{case}-0"
        if results[0]["clipboard"] != expected:
            raise ValueError(f"clipboard mismatch: {case}")
        if len(executions) != 1 or executions[0]["result"] != "executed" or executions[0]["action"] != ("Copy" if copy else "Paste"):
            raise ValueError(f"native execution mismatch: {case}")
        if not captures[0]["time"] < executions[0]["time"] < results[0]["time"]:
            raise ValueError(f"execution outside captured interval: {case}")
        if case.endswith("core"):
            semantic = [r for r in rows if r.get("kind") == "terminal-input"]
            if [r["variant"] for r in semantic] != ([] if copy else ["Paste"]):
                raise ValueError(f"core semantic event mismatch: {case}")
            destination = core_intervals(wire)[(case, 0)]["destination"]
            if destination != "t1" or any(r["terminal"] != destination for r in semantic):
                raise ValueError(f"core semantic/wire destination mismatch: {case}")
            expected_bytes = b"" if copy else b"\x1b[200~" + expected.encode() + b"\x1b[201~"
            if core_segment(wire, case) != expected_bytes:
                raise ValueError(f"core exact bytes mismatch: {case}")
        else:
            interval = intervals.get((case, 0))
            if interval is None or interval["destination"] != "w1:p1" or interval["events"] != ([] if copy else [("w1:p1", {"Paste": expected})]):
                raise ValueError(f"peer exact events mismatch: {case}")
    return "SH1-152 basic receiving-system PASS: six real menu cases"
