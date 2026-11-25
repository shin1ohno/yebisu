"""Synthetic tests of scroll-check.py's parser and pass criteria. These never claim a Simulator run."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("scroll_check", Path(__file__).with_name("scroll-check.py"))
scroll = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scroll)


def mouse(kind, lines):
    return {"Mouse": {"kind": kind, "position": {"Cell": {"column": 3, "row": 4}},
                      "geometry": None, "modifiers": 0, "lines": lines}}


def pane(*events):
    return {"ClientShellPaneInput": {"pane_id": "w1:p1", "events": list(events)}}


def text(value):
    return pane({"TextCommit": value})


def passing():
    return ([pane(mouse("ScrollUp", 5)), pane(mouse("ScrollUp", 9)), text("s")]
            + [pane(mouse("ScrollDown", 4)) for _ in range(6)] + [text("t"), text("u")])


class ScrollCheck(unittest.TestCase):
    def test_events_read_wheel_and_text_and_ignore_the_rest(self):
        self.assertEqual(scroll.events(pane(mouse("ScrollUp", 3), {"TextCommit": "s"}, {"Paste": "x"})),
                         [("mouse", "ScrollUp", 3), ("text", "s")])
        self.assertEqual(scroll.events({"ClientShellEndpointRequest": {"request": "{}"}}), [])
        self.assertEqual(scroll.events({"a": 1, "b": 2}), [])
        self.assertEqual(scroll.events([]), [])

    def test_split_cuts_at_each_marker_in_order(self):
        flat, steps = scroll.split(passing())
        self.assertEqual(len(steps["s"]), 2)
        self.assertEqual(len(steps["t"]), 6)
        self.assertEqual(steps["u"], [])
        self.assertEqual(steps["rest"], [])
        # A marker out of order is input, not a cut.
        _, steps = scroll.split([text("t"), text("s")])
        self.assertEqual(steps["s"], [("text", "t")])
        self.assertNotIn("t", steps)
        self.assertEqual(len(flat), 11)

    def test_a_run_that_matches_the_issue_passes(self):
        _, steps = scroll.split(passing())
        self.assertTrue(all(ok for ok, _ in scroll.verdicts(steps).values()))

    def test_s_fails_on_the_wrong_direction_or_line_count(self):
        for records in ([pane(mouse("ScrollDown", 14)), text("s")],
                        [pane(mouse("ScrollUp", 9)), text("s")],
                        [pane(mouse("ScrollUp", 21)), text("s")],
                        [text("s")]):
            _, steps = scroll.split(records)
            self.assertFalse(scroll.verdicts(steps)["s"][0], records)

    def test_t_needs_scroll_down_in_more_events_than_s(self):
        base = [pane(mouse("ScrollUp", 7)), pane(mouse("ScrollUp", 7)), text("s")]
        _, steps = scroll.split(base + [pane(mouse("ScrollDown", 14)), text("t")])
        self.assertFalse(scroll.verdicts(steps)["t"][0])
        _, steps = scroll.split(base + [pane(mouse("ScrollUp", 1))] * 3 + [text("t")])
        self.assertFalse(scroll.verdicts(steps)["t"][0])
        _, steps = scroll.split(base + [pane(mouse("ScrollDown", 1))] * 3 + [text("t")])
        self.assertTrue(scroll.verdicts(steps)["t"][0])

    def test_u_fails_on_any_wheel_input_or_a_missing_marker(self):
        _, steps = scroll.split(passing()[:-1] + [pane(mouse("ScrollLeft", 1)), text("u")])
        self.assertFalse(scroll.verdicts(steps)["u"][0])
        _, steps = scroll.split(passing()[:-1])
        self.assertFalse(scroll.verdicts(steps)["u"][0])


if __name__ == "__main__":
    unittest.main()
