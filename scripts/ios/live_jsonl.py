"""Read JSONL that another process may still be writing.

The app appends one record per line while the harness copies or reads the file.
A reader can therefore see a final line that is only partly written and does not
end with a newline. Only newline-terminated lines are complete records: they are
all parsed, and a malformed one still raises. The unterminated tail is dropped.
Line counts used as offsets must use the same rule, so use count_lines.
"""
import json
from pathlib import Path

# Records the app writes in place of records it could not deliver
# (crates/yebisu/src/observation.rs): a file holding one is incomplete.
GAP_KINDS = ("observation-dropped", "observation-limit")


def reject_gaps(records):
    """Raise ValueError if the app reported a dropped or unwritten record; else return records."""
    for record in records:
        if isinstance(record, dict) and record.get("kind") in GAP_KINDS:
            raise ValueError(f"app observation gap: {json.dumps(record, sort_keys=True)}")
    return records


def complete_lines(data):
    """Return the newline-terminated lines of data (bytes or str), without newlines."""
    separator = b"\n" if isinstance(data, bytes) else "\n"
    # The last element is "" after a trailing newline, else the unterminated fragment.
    return data.split(separator)[:-1]


def parse(data):
    """Parse every complete line as JSON; an unterminated final fragment is dropped."""
    return reject_gaps([json.loads(line) for line in complete_lines(data)])


def read_records(path, skip=0):
    """Parse the complete lines of the file at path, after the first skip lines.

    Reads bytes so a fragment that stops inside a multi-byte character is never decoded.
    """
    return reject_gaps([json.loads(line) for line in complete_lines(Path(path).read_bytes())[skip:]])


def count_lines(path):
    """Number of complete lines in path, 0 if it does not exist; the offset for read_records."""
    path = Path(path)
    return len(complete_lines(path.read_bytes())) if path.exists() else 0


def heartbeat_gaps(records, interval=2.0, factor=2.5):
    """Where the app's main thread wrote no heartbeat for more than interval * factor seconds.

    The app writes `{"kind": "heartbeat"}` every `interval` seconds from GPUI's foreground
    thread (crates/yebisu/src/qa_ax_input.rs), so a stalled main thread leaves a gap. Records
    after a `launch` record belong to its `pid`; heartbeats are compared within one launch, and
    the time from a launch's last heartbeat to its last record is checked too (`trailing`).
    Returns [{"from", "to", "seconds", "pid"}]; reported by the checks, not judged."""
    limit = interval * factor
    launches = {}
    pid = None
    for record in records:
        if not isinstance(record, dict):
            continue
        if record.get("kind") == "launch":
            pid = record.get("pid")
        time = record.get("time")
        if not isinstance(time, (int, float)):
            continue
        beats, last = launches.setdefault(pid, ([], [time]))
        last[0] = max(last[0], time)
        if record.get("kind") == "heartbeat":
            beats.append(time)
    gaps = []
    for pid, (beats, last) in launches.items():
        beats.sort()
        for start, end in zip(beats, beats[1:]):
            if end - start > limit:
                gaps.append({"from": start, "to": end, "seconds": round(end - start, 2), "pid": pid})
        if beats and last[0] - beats[-1] > limit:
            gaps.append({"from": beats[-1], "to": last[0], "seconds": round(last[0] - beats[-1], 2), "pid": pid, "trailing": True})
    return gaps
