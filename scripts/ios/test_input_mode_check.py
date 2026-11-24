"""input-mode-check.py's judging of T-IM-S1..S8, from recorded observations alone (#139)."""
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("input_mode_check", Path(__file__).with_name("input-mode-check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)

CARET = [300.0, 200.0, 2.0, 24.0]


def line(step, **mode):
    base = {"text_first_responder": True, "activated": True, "caret_rect": CARET,
            "composition_bounds": [300.0, 200.0, 9.0, 24.0], "accessory_frame": None}
    base.update(mode)
    return "INPUT_MODE_OBS " + json.dumps({"step": step, "input_mode": base, "window": [1032, 1376]})


def obs(*lines):
    return check.observations("\n".join(lines))


class ObservationTests(unittest.TestCase):
    def test_lines_are_read_and_a_repeated_step_keeps_the_last(self):
        found = obs("noise", line("a", activated=False), "  " + line("a"), "INPUT_MODE_OBS not json")
        self.assertEqual(list(found), ["a"])
        self.assertTrue(check.active(found, "a"))

    def test_adjacency_is_the_fixture_rule(self):
        self.assertTrue(check.adjacent([272, 227, 157, 43], CARET))  # the probe's 3 pt gap
        self.assertFalse(check.adjacent([272, 240, 157, 43], CARET))  # 16 pt below
        self.assertFalse(check.adjacent([600, 227, 157, 43], CARET))  # no horizontal overlap
        self.assertFalse(check.adjacent(None, CARET))

    def test_caret_follows_within_half_a_point(self):
        self.assertTrue(check.follows(obs(line("s", composition_bounds=[300.4, 199.6, 9, 24])), "s"))
        self.assertFalse(check.follows(obs(line("s", composition_bounds=[301.0, 200, 9, 24])), "s"))


class CaseTests(unittest.TestCase):
    def test_s1_needs_two_of_three_at_the_caret(self):
        shown = [272, 227, 108, 43]
        ok, _ = check.judge_s1(obs(line("s1-focused"), line("s1-switch-1", accessory_frame=shown),
                                   line("s1-switch-2", accessory_frame=shown), line("s1-switch-3")))
        self.assertTrue(ok)
        ok, reason = check.judge_s1(obs(line("s1-focused"), line("s1-switch-1", accessory_frame=shown),
                                        line("s1-switch-2"), line("s1-switch-3")))
        self.assertFalse(ok, reason)
        ok, reason = check.judge_s1(obs(line("s1-focused", activated=False),
                                        *[line(f"s1-switch-{n}", accessory_frame=shown) for n in (1, 2, 3)]))
        self.assertFalse(ok)
        self.assertIn("precondition", reason)

    def test_s5_wants_off_while_a_sheet_is_open_and_on_after(self):
        rows = [line(f"{row}-open", activated=False) for row in check.OFF_WHILE_OPEN]
        rows += [line(f"{row}-closed") for row in check.OFF_WHILE_OPEN]
        self.assertTrue(check.judge_s5(obs(*rows))[0])
        rows[0] = line("s5-settings-open")
        ok, reason = check.judge_s5(obs(*rows))
        self.assertFalse(ok)
        self.assertIn("s5-settings: on while open", reason)

    def test_s4_wants_the_same_marked_text_after_output(self):
        before, after = [10, 40, 9, 18], [10, 58, 9, 18]
        self.assertTrue(check.judge_s4(obs(line("s4-composing", marked="にほん", cursor=before),
                                            line("s4-after-output", marked="にほん", cursor=after)))[0])
        self.assertFalse(check.judge_s4(obs(line("s4-composing", marked="にほん", cursor=before),
                                             line("s4-after-output", marked="", cursor=after)))[0])
        ok, reason = check.judge_s4(obs(line("s4-composing", marked="にほん", cursor=before),
                                        line("s4-after-output", marked="にほん", cursor=before)))
        self.assertFalse(ok)
        self.assertIn("no output", reason)
        ok, reason = check.judge_s4(obs(line("s4-composing", marked=""), line("s4-after-output", marked="")))
        self.assertFalse(ok)
        self.assertIn("not observable", reason)

    def test_s6_and_s7(self):
        self.assertTrue(check.judge_s6(obs(line("s6-keyboard"), line("s6-latched"), line("s6-after-letter"),
                                            line("s6-hidden", activated=False, text_first_responder=False)))[0])
        self.assertFalse(check.judge_s6(obs(line("s6-keyboard"), line("s6-latched", activated=False),
                                             line("s6-after-letter"), line("s6-hidden", activated=False)))[0])
        self.assertTrue(check.judge_s7(obs(line("s7-prefix-armed"),
                                            line("s7-switch", accessory_frame=[272, 227, 108, 43])))[0])
        self.assertFalse(check.judge_s7(obs(line("s7-prefix-armed"), line("s7-switch")))[0])

    def test_s8_finds_a_two_point_caret_and_passes_without_one(self):
        def screen(blue_columns):
            def pixels(x, y):
                return (0, 122, 255) if x in blue_columns and 400 <= y < 448 else (35, 48, 63)
            return (2064, 2752, pixels)
        record = obs(line("s8-cursor"))
        self.assertTrue(check.judge_s8(record, [screen(set()), screen(set())])[0])
        ok, reason = check.judge_s8(record, [screen(set()), screen({600, 601, 602, 603})])
        self.assertFalse(ok, reason)
        self.assertIn("2.0 pt", reason)
        self.assertFalse(check.judge_s8(record, [])[0])


if __name__ == "__main__":
    unittest.main()
