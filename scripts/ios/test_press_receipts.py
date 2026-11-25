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


P0 = json.loads((HERE / "fixtures/press-receipts-ci-p0-cancelled.json").read_text())


class P0TouchCountTests(unittest.TestCase):
    """SmokeQA+Terminal.swift:48 failed "2" is not equal to "1" in runs 45574153676 (main d0aa07c), 45585296675 and
    45578440109: the first P0 touch was cancelled by UIKit, QAAx.press resent it once, the second touch ended."""

    def test_the_first_touch_was_cancelled_so_the_resend_was_proven(self):
        for data in P0["cases"]:
            with self.subTest(case=data["name"]):
                # QAAx.press waits 5 s for presses[id]; the tap went out just before the first touch began.
                self.assertEqual(rule.replay(data["records"], -0.01, 5), rule.CANCELLED)
                self.assertLess(5, data["resend"]["time"])
                self.assertEqual((data["resend"]["kind"], data["resend"]["id"]), ("press-retry", "qa.p0.tap"))
                self.assertIn("cancelled by UIKit before it ended", data["resend"]["reason"])

    def test_the_whole_press_agrees_with_its_one_resend(self):
        for data in P0["cases"]:
            with self.subTest(case=data["name"]):
                counts = rule.touch_counts(data["records"], -0.01, data["records"][-1]["time"])
                self.assertEqual(counts, {"began": 2, "ended": 1, "cancelled": 1})
                self.assertIsNone(rule.touches_mismatch(counts["began"], counts["ended"], counts["cancelled"], resends=1))
                # The check before this fix, touch_began +1 exactly, failed these presses.
                self.assertNotEqual(counts["began"], 1)

    def test_touches_that_do_not_agree_are_named(self):
        mismatch = rule.touches_mismatch
        self.assertIsNone(mismatch(1, 1, 0, 0), "one tap")
        self.assertIsNone(mismatch(1, 1, 0, 1), "lost before the app, then resent")
        self.assertEqual(mismatch(2, 1, 1, 0), "touch_began +2, touch_ended +1, touch_cancelled +1, 0 proven resend(s): "
                                               "a touch UIKit cancelled that no resend answered")
        self.assertEqual(mismatch(2, 2, 0, 1), "touch_began +2, touch_ended +2, touch_cancelled +0, 1 proven resend(s): "
                                               "one press must end in exactly one touch")
        self.assertEqual(mismatch(2, 1, 0, 1), "touch_began +2, touch_ended +1, touch_cancelled +0, 1 proven resend(s): "
                                               "a touch began that neither ended nor was cancelled")
        self.assertIsNotNone(mismatch(1, 0, 1, 1), "resent, and still no completed touch")

    def test_the_swift_check_says_the_same_and_p0_uses_it(self):
        swift = (HERE / "smoke-qa/Tests/QAAx.swift").read_text()
        body = swift[swift.index("static func mismatch(began: Int, ended: Int, cancelled: Int, resends: Int) -> String? {"):]
        body = body[:body.index("\n    }\n")]
        self.assertIn('if ended != 1 { return counts + ": one press must end in exactly one touch" }', body)
        self.assertIn('if began != ended + cancelled { return counts + ": a touch began that neither ended nor was cancelled" }', body)
        self.assertIn('if cancelled > resends { return counts + ": a touch UIKit cancelled that no resend answered" }', body)
        self.assertTrue(body.rstrip().endswith("return nil"))
        p0 = (HERE / "smoke-qa/Tests/SmokeQA+Terminal.swift").read_text()
        test = p0[p0.index("func testInputAndLifecycleReceipts()"):]
        test = test[:test.index("app.typeText(")]
        self.assertIn("let retried = qa.session.retries.count", test)
        self.assertLess(test.index("let retried = qa.session.retries.count"), test.index("try qa.press(QAIds.p0Tap"))
        self.assertIn("PressTouches.mismatch(before: touches, after: touched, resends: qa.session.retries.count - retried)", test)
        self.assertIn("XCTAssertNil(mismatch,", test)
        self.assertNotIn('touches.inputCount("touch_began") + 1', test, "touch_began +1 exactly failed a proven resend")


if __name__ == "__main__":
    unittest.main()
