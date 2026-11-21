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

    def test_portrait_is_observed_after_launch_before_the_first_touch(self):
        launch = body(self.code, "private func launch(")
        self.assertNotIn("XCUIDevice.shared.orientation", launch)
        observed = launch.index("try requirePortraitScreen(name)")
        self.assertLess(launch.index("app.launch()"), observed)
        self.assertLess(launch.index("try ax.waitState("), observed)
        self.assertLess(observed, launch.index("ax.touchReady("))

        portrait = body(self.code, "private func requirePortraitScreen(")
        self.assertRegex(portrait, r"if initial\.width > 0 && initial\.height > initial\.width \{[^}]*\breturn\b")
        self.assertRegex(portrait, r"if initial\.height > 0 && initial\.width > initial\.height \{\s*XCUIDevice\.shared\.orientation = \.portrait\s*\}")
        self.assertEqual(self.code.count("XCUIDevice.shared.orientation = .portrait"), 1)
        self.assertIn("screen.width > 0 && screen.height > screen.width", portrait)
        self.assertIn("guard XCTWaiter().wait(for: [portrait]", portrait)
        self.assertIn("throw SetupError.portraitScreenUnavailable", portrait)

    def test_basic_uses_app_window_points_after_observing_the_full_screen(self):
        basic = body(self.code, "func testSH1152Basic(")
        self.assertIn("try launch(name, useWindowFrames: true, beginOnReady: beginOnReady)", basic)
        launch = body(self.code, "private func launch(")
        self.assertIn('app.launchEnvironment["YEBISU_QA_AX_FRAMES"] = "1"', launch)
        self.assertLess(launch.index("if useWindowFrames && !full"), launch.index("ax.touchReady("))
        qaax = code((TESTS / "QAAx.swift").read_text())
        frame = body(qaax, "private func windowFrame(")
        self.assertIn("$0.active && $0.windowFrame(id) != nil", frame)
        coordinate = body(qaax, "func coordinate(")
        self.assertIn("abs(reported.width - expected.width) < 1", coordinate)
        self.assertLess(coordinate.index("QAAX input frame"), coordinate.index("if framesAgree"))
        self.assertIn(r"XCTest=\(reported) GPUI in screen=\(expected)", coordinate)
        self.assertIn("app.coordinate(withNormalizedOffset: .zero).withOffset", coordinate)
        self.assertIn("try coordinate(empty, id: id", body(qaax, "func touchReady("))

    def test_basic_e1_text_waits_for_the_launch_begin_at_the_peer(self):
        basic = body(self.code, "func testSH1152Basic(")
        self.assertIn('let beginOnReady = name == "E1-text"', basic)
        self.assertIn("if !beginOnReady { try begin(QAIds.sh152Begin, ax) }", basic)
        launch = body(self.code, "private func launch(")
        self.assertIn('if beginOnReady { app.launchEnvironment["YEBISU_SH1_152_BEGIN_ON_READY"] = "1" }', launch)
        receipt = launch.index('$0.count("sh152_start") == 1 && $0.peerPastes.contains(marker)')
        self.assertLess(receipt, launch.index("ax.touchReady("))

    def test_controls_are_pressed_by_identifier_through_qa_press(self):
        press = body(self.code, "private func press(")
        self.assertIn("ax.settleIfMoved(", press)
        self.assertNotIn("ax.settledKeyboard(", press)
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
        self.assertRegex(menu, r"ax\.coordinate\(terminal, id: QAIds\.terminalInput, at: [^\n]*\)\.press\(forDuration: 0\.8\)")
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


class S4FollowUps(unittest.TestCase):
    """UI test redesign S5 does the S4 follow-ups of PR 81's reviews (S4 report section 7.1)."""

    def setUp(self):
        self.edit = code(EDIT_MENU)
        self.smoke = code(SMOKE)
        self.qaax = code((TESTS / "QAAx.swift").read_text())

    def test_review_s2_the_keyboard_settles_only_after_a_step_that_may_move_it(self):
        settle = body(self.qaax, "func settleIfMoved(")
        self.assertIn("guard session.keyboardMayMove else", settle)
        self.assertIn("settledKeyboard(what", settle)
        self.assertIn("session.keyboardMayMove = false", body(self.qaax, "func settledKeyboard("))
        # Every press and the long press go through settleIfMoved; no plain settle before a press.
        self.assertIn("ax.settleIfMoved(", body(self.edit, "private func menu("))
        self.assertIn("qa.settleIfMoved(", body(self.smoke, "func pressSettled("))
        self.assertNotIn("settledKeyboard(", body(self.smoke, "func pressSettled("))
        # Each step that moved the keyboard between two settles in the 681 settles of the three Mac
        # mini runs of 48c8b55 (launch, Begin, Fresh, Mutate, Restore, the E1 raise, a long press)
        # marks it, and so do the gestures and scene changes that can.
        for signature in ("private func begin(", "private func fresh(", "private func mutate(", "private func menu(",
                          "private func keyboardUpBeforePress(", "private func dismissTap(", "private func overlay(",
                          "private func leaveAndReturn(", "private func second(", "private func closeOther("):
            with self.subTest(step=signature):
                self.assertIn("keyboardMayMove(", body(self.edit, signature))
        self.assertIn('try ax.settledKeyboard("\\(name) after Restore")', self.edit)
        self.assertIn("var keyboardMayMove = true", self.qaax, "a launch may move the keyboard")
        prepare = body(self.smoke, "func editPrepare(")
        self.assertIn("movesKeyboard: true", prepare)
        self.assertIn("QAIds.nativeNext, movesKeyboard: true", self.smoke)
        self.assertIn("QAIds.editSelect, movesKeyboard: false", self.smoke)
        self.assertIn("QAIds.editExecute, movesKeyboard: false", self.smoke)

    def test_review_s1_the_scheduled_selector_and_the_paste_are_two_waits(self):
        ran = body(self.smoke, "func scheduledActionsRan(")
        waits = re.findall(r"logWait\(", ran)
        self.assertEqual(len(waits), 2, "one wait for the selector, one for the paste")
        self.assertNotRegex(ran, r"native_actions.*&&.*pasting|pasting.*&&.*native_actions",
                            "the combined predicate that ran out at 5 s in six cases is gone")
        self.assertIn('seconds: reachesApp ? 5 : 1', ran)
        self.assertIn('logWait(qa, "\\(what): no paste pending", seconds: 5) { $0.bool("pasting") == false }', ran)
        self.assertIn("step % 5 != 4", body(self.smoke, "static func editSelectorReachesApp("))
        self.assertIn("step % 6 != 3", body(self.smoke, "static func nativeSelectorReachesApp("))
        self.assertIn("reachesApp: Self.editSelectorReachesApp(step)", self.smoke)
        self.assertIn("reachesApp: Self.nativeSelectorReachesApp(step)", self.smoke)

    def test_review_s2_selector_cases_match_the_app(self):
        # P8 schedules selectAll: at step % 6 == 3, SH1-153 clears the selection at step % 5 == 4.
        native = (Path(__file__).resolve().parents[2] / "crates/yebisu/src/native_edit_qa.rs").read_text()
        self.assertIn('["paste:","copy:","cut:","selectAll:","physical-v","physical-c"][step%6]', native)
        menu = (Path(__file__).resolve().parents[2] / "crates/yebisu/src/edit_menu_qa.rs").read_text()
        self.assertIn("if step % 5 == 4 {\n                                this.handoff.selection = None;", menu)

    def test_rereview_s1_the_image_put_comment_claims_only_what_was_measured(self):
        handoff = (TESTS / "SmokeQA+Handoff.swift").read_text()
        comment = handoff[handoff.index("// An image put"):handoff.index("let imagePut")]
        self.assertNotIn("each\n        // time", comment)
        self.assertNotIn("finished each", comment)
        self.assertNotIn("three times the slowest", comment)
        for fact in ("Whether those two finished is not known", "9.14 s", "44523467982", "not a measured bound"):
            self.assertIn(fact, comment)


class TouchReadyAfterEveryLaunch(unittest.TestCase):
    """PR 84 run 44618790826: E3f-text's first press after its relaunch reached nothing twice (the
    recording showed the window still black). Every EditMenuQA launch now waits for a touch on the
    strip's empty space first, as P12 does."""

    def test_every_launch_goes_through_launch_and_waits_for_a_touch(self):
        source = code(EDIT_MENU)
        self.assertEqual(source.count("app.launch()"), 1, "one launch path")
        launch = body(source, "private func launch(")
        self.assertIn("try ax.touchReady(QAIds.sh152Empty", launch)
        self.assertLess(launch.index('frame("launch'), launch.index("touchReady"))
        self.assertLess(launch.index("touchReady"), launch.index("return (app, ax)"))
        self.assertIn('XCTFail("SH1-152 no touch reached GPUI after launch', launch)

    def test_touch_ready_presses_until_gpui_counts_it(self):
        qaax = code((TESTS / "QAAx.swift").read_text())
        ready = body(qaax, "func touchReady(")
        self.assertIn("for attempt in 1...presses", ready)
        self.assertIn("$0.presses(id) > before.presses(id)", ready)
        self.assertIn("QA_TOUCH_READY_DROP", qaax)
        self.assertNotIn("isHittable", ready)

    def test_the_empty_space_is_marked_and_stops_the_touch(self):
        rust = (Path(__file__).resolve().parents[2] / "crates/yebisu/src/edit_menu_qa_sh1_152.rs").read_text()
        at = rust.index('.id("sh1-152-empty")')
        strip = rust[at:at + 800]
        for token in ("stop_propagation", "on_mouse_down", "on_mouse_up", "on_click", "ids::SH1_152_EMPTY"):
            self.assertIn(token, strip)
        self.assertNotIn("sh1_152_command", strip)


if __name__ == "__main__":
    unittest.main()
