"""Replays real app receipts through QAAx.press's receipt rule (issue #142 stage 3, owner decision on cancelled touches)."""
import importlib.util
import json
from pathlib import Path
import re
import unittest

HERE = Path(__file__).parent
spec = importlib.util.spec_from_file_location("press_receipts", HERE / "press_receipts.py")
rule = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rule)
FIXTURE = json.loads((HERE / "fixtures/press-receipts-ci-45388075978.json").read_text())


def case(prefix):
    return next(c for c in FIXTURE["cases"] if c["name"].startswith(prefix))


class ReplayTests(unittest.TestCase):
    def test_the_two_cancelled_sh1_152_touches_are_proven_lost(self):
        """CI run 45388075978: UIKit cancelled the touch 0.18 s and 0.2 s after it began; the old resend 5.6 s later worked."""
        for prefix in ("sh1-152 E3f-key", "sh1-152 E3g-text"):
            with self.subTest(case=prefix):
                data = case(prefix)
                self.assertEqual(rule.replay(data["records"], -0.01, data["timeout"]), rule.CANCELLED)
                self.assertEqual(rule.touch_counts(data["records"], -0.01, data["timeout"]), {"began": 1, "ended": 0, "cancelled": 1})

    def test_the_sh1_151_touch_after_the_scroll_arrived_and_is_never_sent_again(self):
        data = case("SH1-151")
        self.assertEqual(rule.replay(data["records"], -0.01, data["timeout"]), rule.ARRIVED)

    def test_no_touch_at_all_is_proven_lost(self):
        self.assertEqual(rule.replay([], 0, 5), rule.LOST)
        self.assertEqual(rule.verdict(0, 0, 0), rule.LOST)
        self.assertEqual(rule.verdict(2, 1, 1), rule.ARRIVED, "one touch completed: not lost")

    def test_the_swift_rule_says_the_same(self):
        """The mirror and QAAx.swift's PressVerdict.of must not drift apart."""
        swift = (HERE / "smoke-qa/Tests/QAAx.swift").read_text()
        rule_text = swift[swift.index("static func of(before: QAState, after: QAState?, id: String) -> PressVerdict {"):]
        rule_text = rule_text[:rule_text.index("\n    }\n")]
        self.assertIn('if after.inputCount("touch_began") > before.inputCount("touch_began") {', rule_text)
        self.assertIn('let ended = after.inputCount("touch_ended") - before.inputCount("touch_ended")', rule_text)
        self.assertIn('let cancelled = after.inputCount("touch_cancelled") - before.inputCount("touch_cancelled")', rule_text)
        self.assertRegex(rule_text, r"if ended == 0 && cancelled > 0 \{ return \.cancelled\(lastTouch: after\.lastTouchText\) \}")
        self.assertLess(rule_text.index(".cancelled("), rule_text.index(".arrivedNotCounted("))
        self.assertTrue(rule_text.rstrip().endswith("return .lostProven"))


if __name__ == "__main__":
    unittest.main()
