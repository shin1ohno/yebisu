"""Static guard: the observer poll must not record XCTest failures while waiting.

Swift cannot be compiled on the Linux CI host. This reads SmokeQA.swift as text so
the P11 collector's Text case cannot regress to using the asserting tapText helper
as a presence probe (5749747: 49 XCTFail records, case aborted although the app
had completed all 24 native cases)."""
from pathlib import Path
import re
import unittest

SWIFT = Path(__file__).with_name("smoke-qa") / "Tests" / "SmokeQA.swift"


def body(source, signature):
    """Return the brace-balanced body of the first function starting with signature."""
    start = source.index(signature)
    open_at = source.index("{", source.index(")", start))
    depth = 0
    for at in range(open_at, len(source)):
        depth += {"{": 1, "}": -1}.get(source[at], 0)
        if depth == 0:
            return source[open_at : at + 1]
    raise AssertionError("unbalanced function body: " + signature)


class ObserverPollProbe(unittest.TestCase):
    def setUp(self):
        self.source = SWIFT.read_text()

    def test_completion_poll_uses_non_asserting_probe(self):
        loop = body(self.source, "private func observerProbe(")
        poll = re.search(r"for _ in 0\.\.<\d+ \{(.*?)Thread\.sleep", loop, re.S)
        self.assertIsNotNone(poll, "observer poll loop not found")
        # The defect: tapText records XCTFail on every miss, even under try?.
        check = re.search(r"if [^\n]*collected = true", poll.group(1))
        self.assertIsNotNone(check, "completion check not found")
        self.assertNotIn("tapText", check.group(0))
        self.assertIn("textVisible(done)", check.group(0))

    def test_probe_never_records_a_failure(self):
        for signature in ("private func textVisible(", "private func locateText("):
            with self.subTest(signature=signature):
                function = body(self.source, signature)
                self.assertNotIn("XCTFail", function)
                self.assertNotIn("XCTAssert", function)
                self.assertNotIn("save(", function)

    def test_missing_control_still_fails_in_tap_text(self):
        function = body(self.source, "private func tapText(")
        self.assertIn("XCTFail(", function)
        self.assertIn("locateText(", function)

    def test_oracle_conditions_are_unchanged(self):
        # The fix is test-side only: the completion labels the app paints stay exact.
        self.assertIn('"Observer matrix collected"', self.source)
        self.assertIn('"Native matrix collected"', self.source)
        self.assertIn('XCTAssertTrue(collected, "Missing actual callback/worker/receiving completion")', self.source)


if __name__ == "__main__":
    unittest.main()
