"""Synthetic P13 barrier/FIFO oracle tests only. These never claim native execution."""
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p13", Path(__file__).with_name("p13-check.py"))
p13 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p13)

FRESH = [{"TextCommit": "P13-fresh"}, {"TextCommit": "P13-fresh"}]
LABELS = ("old-after-close", "fresh-after-close")


def barrier(label):
    request = {"method": "pane.scroll", "params": {"pane_id": "w1:p1", "offset_from_bottom": 0, "p13_barrier": label}}
    return {"ClientShellEndpointRequest": {"boot_id": "boot-v1", "request": json.dumps(request)}}


def commit():
    return {"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"TextCommit": "P13-fresh"}]}}


def wire(early):
    """Fresh input after the last barrier when `early`, as on the mini 2026-11-16 turn 1."""
    head = [barrier("old-before-close"), barrier("old-after-close")]
    if early:
        return head + [barrier("fresh-after-close"), commit(), commit()]
    return head + [commit(), commit(), barrier("fresh-after-close")]


def records(flushed):
    return [{"kind": "p13-barrier", "label": label, "flushed": flushed, "ready": flushed, "turns": 1} for label in LABELS]


class P13Barrier(unittest.TestCase):
    def test_flushed_barriers_and_in_order_wire_pass(self):
        p13.barriers(records(True), LABELS, "p13-barrier-before-flush")
        p13.fifo_phases(wire(early=False), FRESH)

    def test_barrier_sent_before_flush_is_named(self):
        with self.assertRaisesRegex(ValueError, "^p13-barrier-before-flush: old-after-close"):
            p13.barriers(records(False), LABELS, "p13-barrier-before-flush")

    def test_missing_or_repeated_barrier_record_is_named(self):
        with self.assertRaisesRegex(ValueError, "^p13-barrier-before-flush: fresh-after-close"):
            p13.barriers(records(True)[:1], LABELS, "p13-barrier-before-flush")
        with self.assertRaisesRegex(ValueError, "^p13-barrier-before-flush: old-after-close"):
            p13.barriers(records(True) + records(True)[:1], LABELS, "p13-barrier-before-flush")

    def test_early_barrier_fails_the_wire_even_when_recorded_flushed(self):
        # The record is the probe's own claim; the receiving wire decides.
        p13.barriers(records(True), LABELS, "p13-barrier-before-flush")
        with self.assertRaisesRegex(ValueError, "^D5 receiving FIFO phases"):
            p13.fifo_phases(wire(early=True), FRESH)


if __name__ == "__main__":
    unittest.main()
