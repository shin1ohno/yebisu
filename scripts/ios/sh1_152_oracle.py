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


def core_segment(wire, case, stage=0):
    start = f"\x1b[200~sh1-152-start-{case}-{stage}\x1b[201~".encode()
    end = f"\x1b[200~sh1-152-end-{case}-{stage}\x1b[201~".encode()
    active, destination, found, data = False, None, False, bytearray()
    for row in wire:
        payload = bytes(row["bytes"])
        if payload == start:
            if active or found:
                raise ValueError("duplicate core start")
            active, found, destination = True, True, row["terminal"]
        elif payload == end:
            if not active:
                raise ValueError("unmatched core end")
            active = False
        elif active:
            if row["terminal"] != destination:
                raise ValueError("core input reached another terminal")
            data.extend(payload)
    if active or not found:
        raise ValueError("core interval missing")
    return bytes(data)


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
            expected_bytes = b"" if copy else b"\x1b[200~" + expected.encode() + b"\x1b[201~"
            if core_segment(wire, case) != expected_bytes:
                raise ValueError(f"core exact bytes mismatch: {case}")
        else:
            interval = intervals.get((case, 0))
            if interval is None or interval["destination"] != "w1:p1" or interval["events"] != ([] if copy else [("w1:p1", {"Paste": expected})]):
                raise ValueError(f"peer exact events mismatch: {case}")
    return "SH1-152 basic receiving-system PASS: six real menu cases"
