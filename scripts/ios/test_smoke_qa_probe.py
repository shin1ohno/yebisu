"""Static guard: the observer poll must not record XCTest failures while waiting.

Swift cannot be compiled on the Linux CI host. This reads the SmokeQA*.swift files as text so
the P11 collector's Text case cannot regress to using the asserting tapText helper
as a presence probe (5749747: 49 XCTFail records, case aborted although the app
had completed all 24 native cases)."""
from pathlib import Path
import re
import unittest

TESTS = Path(__file__).with_name("smoke-qa") / "Tests"


def swift_source():
    """SmokeQA.swift and every SmokeQA+<unit>.swift extension, as one text."""
    return "\n".join(path.read_text() for path in sorted(TESTS.glob("SmokeQA*.swift")))


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


def whole_word_range(label, text, next_word=None):
    """Python mirror of SmokeQA.wholeWordRange (SmokeQA.swift), line for line.

    Swift cannot run on the Linux host, so this models the algorithm and
    WholeWordMatch pins its behaviour; test_locate_text_matches_whole_words ties the
    Swift source to the same tokens."""
    start = 0
    while start < len(text):
        at = text.find(label, start)
        if at < 0:
            return None
        end = at + len(label)
        start = at + 1
        if at > 0 and text[at - 1].isalnum():
            continue
        if end < len(text) and text[end].isalnum():
            continue
        if next_word is not None:
            following = re.match(r"\s*([A-Za-z0-9]*)", text[end:]).group(1)
            if following.lower() == next_word.lower():
                continue
        return (at, end)
    return None


class WholeWordMatch(unittest.TestCase):
    def paste(self, text):
        return whole_word_range("Paste", text, "image")

    def test_paste_image_is_not_paste(self):
        self.assertIsNone(self.paste("Paste image"))

    def test_capitalised_image_is_not_paste(self):
        self.assertIsNone(self.paste("Paste Image"))

    def test_first_paste_before_paste_image_matches(self):
        self.assertEqual(self.paste("Paste Paste image"), (0, 5))

    def test_paste_after_another_word_matches(self):
        self.assertEqual(self.paste("Copy Paste"), (5, 10))

    def test_other_cases(self):
        self.assertEqual(self.paste("Paste"), (0, 5))
        self.assertEqual(self.paste("Select Copy Paste Paste image Start"), (12, 17))
        self.assertIsNone(self.paste("Pasted"))
        self.assertIsNone(self.paste("Repaste"))
        self.assertIsNone(self.paste("Copy Paste  image"))
        self.assertEqual(whole_word_range("Paste image", "Paste image"), (0, 11))
        self.assertEqual(whole_word_range("QA Image", "QA Image QA Image only", "only"), (0, 8))
        self.assertIsNone(whole_word_range("QA Image", "QA Image only", "only"))
        self.assertIsNone(whole_word_range("QA Image", "QA image ready", "only"))


class ObserverPollProbe(unittest.TestCase):
    def setUp(self):
        self.source = swift_source()

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
        for signature in ("func textVisible(", "private func locateText("):
            with self.subTest(signature=signature):
                function = body(self.source, signature)
                self.assertNotIn("XCTFail", function)
                self.assertNotIn("XCTAssert", function)
                self.assertNotIn("save(", function)

    def test_missing_control_still_fails_in_tap_text(self):
        function = body(self.source, "func tapText(")
        self.assertIn("XCTFail(", function)
        self.assertIn("locateText(", function)

    def test_sh154_locates_every_strip_control_once_at_the_start(self):
        # 2ed2beb / L15 / L16: a later OCR of the strip can miss "Paste" or read a
        # truncated "Paste image", and the tap then starts the wrong paste. Every
        # strip control is located once, before the first tap, and tapped by point.
        test = body(self.source, "func testSH154CoreImage(")
        calls = re.findall(r'tapText\("([^"]+)"([^)]*)\)', test)
        located = [(label, rest) for label, rest in calls if "tap: false" in rest and "attempts" not in rest]
        self.assertEqual([label for label, _ in located], ["QA Image", "Paste", "Paste image"])
        self.assertEqual([label for label, rest in calls if "attempts: 15" in rest], ["QA Image only"])
        # A tapText call that taps (no `tap: false`) would be a fresh OCR of the strip.
        self.assertEqual([label for label, rest in calls if "tap: false" not in rest], [])
        self.assertIn('excludingNextWord: "only"', test)
        self.assertIn('excludingNextWord: "image"', test)
        first_tap = test.index("tapRecorded(")
        for label in ("QA Image only", "QA Image", "Paste", "Paste image"):
            self.assertLess(test.index('tapText("%s"' % label), first_tap, label)
        taps = re.findall(r"tapRecorded\((\w+),", test)
        self.assertEqual(taps, ["qaImageOnly", "pasteImage", "paste", "qaImage", "paste", "pasteImage"])
        # L2b: Paste and Paste image are ... menu items, so every one of their taps
        # reopens the menu at the point that opened it first, never by a fresh OCR.
        for box in re.findall(r"openOverflowMenu\(in: app, at: more\)\n\s*tapRecorded\((\w+),", test):
            self.assertIn(box, ("paste", "pasteImage"))
        self.assertEqual(len(re.findall(r"openOverflowMenu\(in: app, at: more\)\n\s*tapRecorded\(", test)), 4)
        self.assertEqual(test.count("openOverflowMenu(in: app)"), 1)
        # Waits stay as they were: every tap is followed by the state it must produce.
        waits = re.findall(r'tapText\("(paste-\d\.png|QA image ready)", in: app, tap: false, attempts: 30\)', test)
        self.assertEqual(waits, ["QA image ready", "paste-0.png", "paste-1.png", "QA image ready", "paste-2.png"])
        # A misread box is rejected at the start, not only by the final put/path/text counts.
        self.assertIn("left.minX < right.minX", test)
        self.assertLess(test.index("left.minX < right.minX"), first_tap)

    def test_locate_text_matches_whole_words(self):
        self.assertNotIn("range(of: label)", body(self.source, "private func locateText("))
        self.assertIn("SmokeQA.wholeWordRange(", body(self.source, "private func locateText("))
        self.assertNotIn("excludingSuffix", self.source)
        function = body(self.source, "static func wholeWordRange(")
        for token in ("isLetter", "isNumber", "isWhitespace", "lowercased()", "while from < text.endIndex"):
            self.assertIn(token, function)

    def test_oracle_images_check_is_unchanged(self):
        checker = (Path(__file__).with_name("p9-check.py")).read_text()
        self.assertIn("(3, 3, 1, None)", checker)

    def test_oracle_conditions_are_unchanged(self):
        # The fix is test-side only: the completion labels the app paints stay exact.
        self.assertIn('"Observer matrix collected"', self.source)
        self.assertIn('"Native matrix collected"', self.source)
        self.assertIn('XCTAssertTrue(collected, "Missing actual callback/worker/receiving completion")', self.source)


if __name__ == "__main__":
    unittest.main()
