"""Static guards on the SmokeQA tests' probes (Swift cannot be compiled on the Linux CI host).

The P11 collector's poll must not record XCTest failures while it waits (5749747: 49 XCTFail
records from the asserting OCR tap, case aborted although the app had completed all 24 native
cases). Since the UI test redesign S6 the poll reads the app's phase in qa.state, and the OCR
helpers (locateText, recognize, wholeWordRange, ocrLineCount, the tapText family) are gone."""
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


HELPERS = ("locateText", "recognize", "wholeWordRange", "ocrLineCount", "tapText", "tapTextBox", "tapRecorded",
           "tapMenuItem", "textVisible", "waitForRenderedText", "openSettings", "openOverflowMenu",
           "closeOverflowMenu", "hideSoftwareKeyboard", "scrollBoard")


def all_swift():
    """Every Swift UI test file: SmokeQA, EditMenuQA, the QAAx helpers and P12QA."""
    files = sorted(TESTS.glob("*.swift")) + [Path(__file__).with_name("p12-qa") / "Tests" / "P12QA.swift"]
    return {path.name: path.read_text() for path in files}


class OcrHelpersRemoved(unittest.TestCase):
    """S6: the OCR helpers are removed and nothing calls them."""
    def test_no_definition_and_no_caller(self):
        for name, text in all_swift().items():
            for helper in HELPERS:
                with self.subTest(file=name, helper=helper):
                    self.assertNotRegex(text, r"\bfunc %s\(" % helper)
                    self.assertNotRegex(text, r"(?<![\w.])%s\(" % helper)
        for constant in ("appearancePage", "terminalInputRow"):
            self.assertNotIn(constant, "\n".join(all_swift().values()))

    def test_negative_control_a_helper_left_behind_is_caught(self):
        text = all_swift()["SmokeQA.swift"] + "\n    func tapText(_ label: String) {}\n"
        self.assertRegex(text, r"\bfunc tapText\(")

    def test_the_labels_check_is_gone_and_ci_checks_identifiers(self):
        here = Path(__file__).parent
        self.assertFalse((here / "smokeqa-labels-check.py").exists())
        ci = (Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml").read_text()
        self.assertNotIn("smokeqa-labels-check", ci)
        self.assertIn("run: python3 scripts/ios/qa-ax-ids-check.py\n", ci)


class ObserverPollProbe(unittest.TestCase):
    def setUp(self):
        self.source = swift_source()

    def test_completion_poll_reads_the_phase_in_qa_state(self):
        loop = body(self.source, "private func observerProbe(")
        poll = re.search(r"for press in 0\.\.<160 \{(.*?)Thread\.sleep", loop, re.S)
        self.assertIsNotNone(poll, "observer poll loop not found")
        # The completion check reads qa.state and records no failure while it waits.
        self.assertIn("if progress.phase > 24 { collected = true; break }", poll.group(1))
        self.assertNotIn("XCTFail", poll.group(1))
        self.assertIn("try qa.press(control", poll.group(1))
        self.assertIn("modal ? QAIds.p11Observer : QAIds.nativeNext", loop)

    def test_sh154_presses_by_identifier_in_the_old_order(self):
        # S3 replaced the once-located OCR boxes (2ed2beb / L15 / L16, F4) with QA identifiers:
        # the band and the ... menu are pressed where the app draws them on that frame. The
        # order of the presses and the state each one must produce stay as before S3.
        test = body(self.source, "func testSH154CoreImage(")
        presses = re.findall(r"(?:qa\.press|menuItem\(qa,) ?\(?(QAIds\.\w+)", test)
        # The layout checks the OCR boxes had are kept on the identifiers' frames.
        self.assertIn("qaImage.minX < qaImageOnly.minX", test)
        self.assertIn("paste.midY < pasteImage.midY", test)
        self.assertEqual(presses, ["QAIds.bandImageOnly", "QAIds.menuPasteImage", "QAIds.menuPaste",
                                   "QAIds.bandImage", "QAIds.menuPaste", "QAIds.menuPasteImage"])
        # Before the first press the band is found and the software keyboard is down
        # (it lifts the band while it rises, Mac mini 32f1e21).
        hidden = test.index('qa.hideKeyboard("SH154 start")')
        self.assertLess(test.index("qa.find(QAIds.bandImageOnly"), hidden)
        self.assertLess(hidden, test.index("try qa.press(QAIds.bandImageOnly"))
        # Every step waits for the app's own count: an image prepared, an image or a text pasted.
        waits = re.findall(r'what: "([^"]+)"', test)
        self.assertEqual(waits, ["QA image ready (image only)", "Paste image put paste-0.png",
                                 "Paste put paste-1.png (the item has no text)", "QA image ready (text and image)",
                                 "Paste sent the text", "Paste image put paste-2.png"])
        for counter in ("qa_images", "pasted_images", "pasted_texts", "paste_failures"):
            self.assertIn('"%s"' % counter, test)

    def test_oracle_images_check_is_unchanged(self):
        checker = (Path(__file__).with_name("p9-check.py")).read_text()
        self.assertIn("(3, 3, 1, None)", checker)

    def test_oracle_conditions_are_unchanged(self):
        # The app still draws the completion labels from the phase the test reads (phase > 24).
        root = Path(__file__).resolve().parents[2] / "crates/yebisu/src"
        for path, label in (("settings_sheet/observer_qa.rs", "Observer matrix collected"),
                            ("native_observer_qa.rs", "Native matrix collected")):
            app = (root / path).read_text()
            self.assertIn(f'"{label}"', app)
            self.assertIn("self.observer.phase > 24", app)
            self.assertIn('"phase": self.observer.phase', app)
        self.assertIn('XCTAssertTrue(collected, "Missing actual callback/worker/receiving completion")', self.source)


if __name__ == "__main__":
    unittest.main()
