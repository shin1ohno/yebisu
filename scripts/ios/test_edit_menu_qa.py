"""Static guards for UI test redesign S4: the edit-menu job's XCTests press QA identifiers and wait
on qa.state.

Swift cannot be compiled on the Linux CI host, so this reads EditMenuQA.swift and the SmokeQA
extensions as text (as test_smoke_qa_waits.py does for S3). It fixes what S4 removed: OCR, the
remembered label positions (`located`), the keyboard-settle polling of XCTest's keyboard frame,
the partial-match lookup of a label an edit menu covers, fixed sleeps, and reading the app's
receipt files while the app runs. It also fixes what the receiving-system oracles still read from
the XCTest log and the screenshots, so the migration cannot drop a case or a stage."""
from pathlib import Path
import re
import unittest

TESTS = Path(__file__).with_name("smoke-qa") / "Tests"
EDIT_MENU = (TESTS / "EditMenuQA.swift").read_text()
SMOKE = "\n".join(path.read_text() for path in sorted(TESTS.glob("SmokeQA*.swift")))
OCR = ("tapText(", "tapTextBox(", "tapMenuItem(", "textVisible(", "waitForRenderedText(", "tapRecorded(",
       "openSettings(in:", "openOverflowMenu(", "closeOverflowMenu(", "recognize(", "hideSoftwareKeyboard(",
       "VNRecognizeTextRequest", "terminalInputRow")


def body(source, signature):
    """The brace-balanced body of the first function starting with signature."""
    start = source.index(signature)
    open_at = source.index("{", source.index(")", start))
    depth = 0
    for at in range(open_at, len(source)):
        depth += {"{": 1, "}": -1}.get(source[at], 0)
        if depth == 0:
            return source[open_at : at + 1]
    raise AssertionError("unbalanced function body: " + signature)


def code(text):
    """The lines that are not comments."""
    return "\n".join(line for line in text.splitlines() if line.strip() and not line.strip().startswith("//"))


class EditMenuQAIdentifiers(unittest.TestCase):
    def setUp(self):
        self.code = code(EDIT_MENU)

    def test_no_ocr_remembered_position_or_keyboard_frame_polling(self):
        for gone in ("import Vision", "VNRecognizeTextRequest", "located", "locatedKeyboard", "keyboardFrame(",
                     "settledKeyboard(app)", "covered(", "remember(", "customWords", "partially covered",
                     "label.hasSuffix(text)", "menuOpen"):
            with self.subTest(gone=gone):
                self.assertNotIn(gone, self.code)

    def test_no_receipt_file_is_read_while_the_app_runs(self):
        for reader in ("SH1_152_DOCUMENTS", "FileManager", "contentsOfDirectory", "String(contentsOf",
                       "JSONSerialization", "records(", "p4-peer.jsonl", "p9-core-wire.jsonl"):
            with self.subTest(reader=reader):
                self.assertNotIn(reader, self.code)

    def test_the_only_fixed_sleep_waits_for_the_closed_scene_session(self):
        sleeps = re.findall(r"\b(?:u?sleep|Thread\.sleep)\([^)]*\)", self.code)
        self.assertEqual(sleeps, ["sleep(3)"], "only the wait for UIKit to destroy the closed scene's session")
        self.assertIn("UIKit destroys the closed scene's session outside the app", EDIT_MENU)

    def test_launch_turns_the_qa_layer_on_and_waits_for_the_input_destination(self):
        launch = body(self.code, "private func launch(")
        self.assertIn('app.launchEnvironment["YEBISU_QA_AX"] = "1"', launch)
        self.assertIn('$0.string("input_pane") != nil', launch)

    def test_controls_are_pressed_by_identifier_through_qa_press(self):
        press = body(self.code, "private func press(")
        self.assertIn("ax.settledKeyboard(", press)
        self.assertIn("ax.press(id, at: offset", press)
        for control in ("QAIds.sh152Begin", "QAIds.sh152Select", "QAIds.sh152Mutate", "QAIds.sh152Replay",
                        "QAIds.sh152Fresh", "QAIds.sh152End", "QAIds.sh152Ordinary", "QAIds.sh152Restore",
                        "QAIds.sh152Second", "QAIds.sh152TwoBegin", "QAIds.sh152TwoSelect", "QAIds.sh152TwoEnd",
                        "QAIds.sh152TwoClose"):
            with self.subTest(control=control):
                self.assertIn(control, self.code)

    def test_the_long_press_is_on_the_terminal_frame_and_never_repeated(self):
        menu = body(self.code, "private func menu(")
        self.assertIn("ax.find(QAIds.terminalInput)", menu)
        self.assertRegex(menu, r"terminal\.coordinate\(withNormalizedOffset: [^\n]*\)\.press\(forDuration: 0\.8\)")
        self.assertEqual(menu.count("press(forDuration:"), 1)
        self.assertNotIn("ax.press(", menu, "QAAx.press presses once more on a lost press; a long press must not")

    def test_cases_and_what_the_oracles_read_are_unchanged(self):
        # sh1-152-check.py requires each method to pass and reads these log lines.
        for method in ("testSH1152Basic", "testSH1152Transitions", "testSH1152DismissAndSystemEdit",
                       "testSH1152TwoScenes"):
            self.assertIn("func %s() throws" % method, self.code)
        cases = re.findall(r'for name in \[([^\]]*)\]', self.code)
        names = [name.strip(' "') for group in cases for name in group.split(",")]
        self.assertEqual(names, ["E1-text", "E1-key", "E2-text", "E2-key", "E1-core", "E2-core",
                                 "E3a", "E3b", "E3c", "E3d", "E3e", "E3f-text", "E3f-key", "E3g-text", "E3g-key",
                                 "E3h", "E3i", "E3a-core", "E4-text", "E4-key", "E7", "E5a", "E5b", "E5c", "E5d"])
        self.assertIn('print("SH1-152 VISIBLE \\(name) selected=\\(selected) labels=\\(labels.joined(separator: ","))")',
                      self.code)
        self.assertIn('print("SH1-152 E7 system Edit Paste selected")', self.code)
        self.assertIn('print("SH1-152 E7 unverified: system Edit not exposed; device item 9")', self.code)
        self.assertIn('XCTAssertEqual(labels.sorted(), selected ? ["Copy", "Paste"] : ["Paste"])', self.code)


class SmokeQAEditMenuIdentifiers(unittest.TestCase):
    """testSH1153EditMenu, testP8NativeEditing, testSH1151SelectionSettings and
    testSH1156PointerClickAfterDiscard."""
    TESTS = {
        "func testSH1153EditMenu(": ["QAIds.editPrepare", "QAIds.editSelect", "QAIds.editExecute"],
        "func testP8NativeEditing(": ["QAIds.nativeNext"],
        "func testSH1151SelectionSettings(": ["QAIds.startDoubleTap", "QAIds.copyOnSelection", "QAIds.settingsClose"],
        "func testSH1156PointerClickAfterDiscard(": ["QAIds.sh156Discard", "QAIds.terminalInput", "QAIds.menuSelectText",
                                                     "QAIds.settingsClose"],
    }
    HELPERS = ("func pressSettled(", "func launchPeerTerminal(", "func editPrepare(", "func openInputPage(",
               "func typed(", "func markersAtPeer(", "func scheduledActionsRan(", "func logWait(")

    def setUp(self):
        self.code = code(SMOKE)

    def test_no_ocr_or_fixed_sleep(self):
        for signature in (*self.TESTS, *self.HELPERS):
            function = body(self.code, signature)
            with self.subTest(function=signature):
                for call in OCR:
                    self.assertNotIn(call, function)
                self.assertNotRegex(function, r"\b(?:u?sleep|Thread\.sleep)\(")

    def test_each_test_presses_its_controls_by_identifier(self):
        for signature, ids in self.TESTS.items():
            function = body(self.code, signature)
            with self.subTest(test=signature):
                self.assertIn("continueAfterFailure = false", function)
                self.assertTrue('launchEnvironment["YEBISU_QA_AX"] = "1"' in function or "launchPeerTerminal(" in function)
                for identifier in ids:
                    self.assertIn(identifier, function + body(self.code, "func editPrepare("))
        self.assertIn('app.launchEnvironment["YEBISU_QA_AX"] = "1"', body(self.code, "func launchPeerTerminal("))

    def test_the_pointer_click_is_a_click_on_the_terminal_and_confirmed(self):
        function = body(self.code, "func testSH1156PointerClickAfterDiscard(")
        self.assertIn("terminal.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click()", function)
        self.assertIn("$0.presses(QAIds.terminalInput) > before.presses(QAIds.terminalInput)", function)

    def test_saved_stages_are_unchanged(self):
        # The drivers copy these screenshots by name.
        for signature, stages in (
                ("func testP8NativeEditing(", ['save("p8-native")']),
                ("func testSH1151SelectionSettings(", ['save("sh1-151-fresh")', 'save("sh1-151-restored")']),
                ("func testSH1156PointerClickAfterDiscard(", ['save("sh1-156-discarded")', 'save("sh1-156-clicked")',
                                                             'save("sh1-156-select-on")', 'save("sh1-156-select-off")',
                                                             'save("sh1-156-typed")', 'save("sh1-156-after-settings")'])):
            function = body(self.code, signature)
            saved = [line.strip() for line in function.splitlines() if line.strip().startswith("save(")]
            self.assertEqual(saved, stages, signature)
        sh153 = body(self.code, "func testSH1153EditMenu(")
        self.assertIn('save("sh1-153-case-\\(step)")', sh153)
        self.assertIn("for step in 0..<10", sh153)
        self.assertIn("for step in 0..<25", body(self.code, "func testP8NativeEditing("))


if __name__ == "__main__":
    unittest.main()
