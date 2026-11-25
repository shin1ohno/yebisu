"""The receipt rule QAAx.press decides a missing press by (smoke-qa/Tests/QAAx.swift, PressVerdict), in Python.

QAAx runs inside XCTest, so its rule cannot replay a CI run's files. This mirror of the receipt part of
PressVerdict.of lets test_press_receipts.py replay the app's own input receipts from a real run through the
same rule (issue #142 stage 3): no touch began → proven lost (send once more); every new touch cancelled by
UIKit before it ended → proven lost too (send once more); a touch that ended in the app → arrived, not counted
(never sent again; fail with last_touch). The screen-field and relaunch checks that come first in Swift are not
part of the receipts and are left out. `touches_mismatch` mirrors PressTouches.mismatch, the check that one
press's touches agree with its proven resends (P0's receipt count).
"""
LOST = "lost-proven"
CANCELLED = "cancelled"
ARRIVED = "arrived-not-counted"


def touch_counts(records, start, end):
    """How many touches began, ended and were cancelled within [start, end] (`time` of input-receipt records)."""
    counts = {"began": 0, "ended": 0, "cancelled": 0}
    for record in records:
        time = record.get("time")
        if record.get("kind") == "input-receipt" and record.get("receipt") == "touch" and isinstance(time, (int, float)) \
                and start <= time <= end and record.get("stage") in counts:
            counts[record["stage"]] += 1
    return counts


def verdict(began, ended, cancelled):
    """PressVerdict's receipt rule for the touch deltas over one press's wait."""
    if began == 0:
        return LOST
    if ended == 0 and cancelled > 0:
        return CANCELLED
    return ARRIVED


def replay(records, start, timeout):
    """The verdict QAAx.press reaches for a press sent at `start` that showed nothing within `timeout` seconds."""
    counts = touch_counts(records, start, start + timeout)
    return verdict(counts["began"], counts["ended"], counts["cancelled"])


def touches_mismatch(began, ended, cancelled, resends):
    """QAAx.swift's PressTouches.mismatch: whether one press's touch deltas agree with the resends proven for it
    (None), or why not. One completed touch; every touch that began ended or was cancelled; each cancelled touch
    answered by a resend (issue #142 follow-up, CI run 45574153676: began, cancelled, press-retry, began, ended)."""
    counts = f"touch_began +{began}, touch_ended +{ended}, touch_cancelled +{cancelled}, {resends} proven resend(s)"
    if ended != 1:
        return counts + ": one press must end in exactly one touch"
    if began != ended + cancelled:
        return counts + ": a touch began that neither ended nor was cancelled"
    if cancelled > resends:
        return counts + ": a touch UIKit cancelled that no resend answered"
    return None
