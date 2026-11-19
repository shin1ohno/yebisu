"""Static guards for the state waits of SmokeQA.testInputAndLifecycleReceipts and the P9 timeout.

Swift cannot be compiled on the Linux CI host, so this reads the SmokeQA*.swift files as text. The
failures it guards (class C of the L17 diagnosis) are a fixed sleep after Home, activate()
or typeText that lets the next step run before the app is ready, and a P9 hang that runs
into the 40-minute job timeout instead of failing the test."""
import importlib.util
from pathlib import Path
import re
import unittest

TESTS = Path(__file__).with_name("smoke-qa") / "Tests"


def swift_source():
    """SmokeQA.swift and every SmokeQA+<unit>.swift extension, as one text."""
    return "\n".join(path.read_text() for path in sorted(TESTS.glob("SmokeQA*.swift")))

spec = importlib.util.spec_from_file_location("p9_check", Path(__file__).with_name("p9-check.py"))
p9 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p9)


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


def statements(function):
    return [line.strip() for line in function.splitlines() if line.strip() and not line.strip().startswith("//")]


class LifecycleWaits(unittest.TestCase):
    """P0 waits on qa.state since the UI test redesign S6 (before: OCR of one word of the input)."""
    def setUp(self):
        self.source = swift_source()
        self.test = statements(body(self.source, "func testInputAndLifecycleReceipts("))

    def following(self, prefix):
        """Statements that directly follow each statement starting with prefix."""
        return [self.test[at + 1] for at, line in enumerate(self.test) if line.startswith(prefix) and at + 1 < len(self.test)]

    def test_no_fixed_sleep_follows_home_activate_or_typing(self):
        for prefix in ("XCUIDevice.shared.press(.home)", "app.activate()", 'app.typeText("P0-input")',
                       'app.typeText("-returned")', 'app.typeText("-rotated")', 'app.typeText("-idle")'):
            with self.subTest(prefix=prefix):
                steps = self.following(prefix)
                self.assertTrue(steps, prefix + " not found")
                for step in steps:
                    self.assertNotIn("sleep(", step)

    def test_each_wait_is_a_named_state_wait(self):
        text = "\n".join(self.test)
        self.assertIn("press(.home)\n" + 'try requireState(app, [.runningBackground, .runningBackgroundSuspended], "app left the foreground after press(.home)")', text)
        self.assertIn('app.activate()\ntry requireState(app, [.runningForeground], "activate() after press(.home)")', text)
        for value in ("P0-input", "P0-input-returned", "P0-input-returned-rotated", "P0-input-returned-rotated-idle"):
            self.assertIn(f'try waitP0Input(qa, "{value}", what:', text)
        # The input values are the oracle's: smoke_oracle.EXPECTED.
        oracle = (Path(__file__).with_name("smoke_oracle.py")).read_text()
        for value in ("P0-input", "P0-input-returned", "P0-input-returned-rotated", "P0-input-returned-rotated-idle"):
            self.assertIn(f'"{value}"', oracle)
        wait = body(self.source, "func waitP0Input(")
        self.assertIn('$0.string("p0_input") == text', wait)

    def test_a_failed_wait_stops_the_test_at_once(self):
        self.assertIn("continueAfterFailure = false", "\n".join(self.test))
        function = body(self.source, "func requireState(")
        self.assertIn("XCTFail(", function)
        self.assertIn("throw NSError(", function)
        self.assertIn("timeout", function)
        # waitP0Input goes through QAAx.waitState, which throws a named timeout.
        self.assertIn("try qa.waitState(", body(self.source, "func waitP0Input("))

    def test_state_wait_asks_xctest_to_refresh_the_state(self):
        # Run 43770292492: polling `app.state` alone kept reporting runningForeground for 30 s
        # after press(.home) and the wait failed although Home had been handled. The wait
        # must go through app.wait(for:timeout:), return as soon as a wanted state is reached,
        # and only fail after its timeout (run 43790640783 shows it passing, 5.29 s after Home).
        function = body(self.source, "func requireState(")
        wait = function.index("app.wait(for: state")
        self.assertLess(wait, function.index("return"))
        self.assertLess(function.index("return"), function.index("XCTFail("))
        self.assertNotRegex(function, r"while\s+!states\.contains\(app\.state\)")

    def test_oracle_stages_and_idle_interval_are_unchanged(self):
        stages = [line for line in self.test if line.startswith("save(")]
        self.assertEqual(stages, ['save("initial")', 'save("returned")', 'save("rotated")',
                                  'save("idle-before")', 'save("idle")', 'save("image-clear")'])
        self.assertIn("holdIdle(qa, seconds: 60)", self.test)
        self.assertIn("XCTAssertEqual(app.state, .runningForeground)", self.test)
        # 6 slices of sleep(10): the 60 s the oracle requires between idle-before and idle.
        idle = body(self.source, "func holdIdle(")
        self.assertIn("stride(from: 10, through: seconds, by: 10)", idle)
        self.assertIn("sleep(10)", idle)

    def test_s6_idle_wait_is_measured_by_name(self):
        """S5 follow-up S6-2: the idle wait is no longer met by an OCR screenshot, and how long
        XCTest waited for the app to idle is printed (run 44547820393 attempt 1: 80 s)."""
        text = "\n".join(self.test)
        idle = body(self.source, "func holdIdle(")
        self.assertIn("qa.read()", idle)
        self.assertIn("slow qa.state reads while idle", idle)
        self.assertIn("P0 idle: typeText('-idle') took", text)
        self.assertLess(text.index("let typing = Date()"), text.index('app.typeText("-idle")'))
        for call in S3Identifiers.OCR + ("XCUIScreen.main.screenshot()", "withNormalizedOffset"):
            self.assertNotIn(call, text)
        # The press on the P0 control and the keyboard's hide key replaced two window points.
        self.assertIn("try qa.press(QAIds.p0Tap", text)
        self.assertIn('try qa.hideKeyboard("P0 image-clear")', text)
        driver = (Path(__file__).with_name("smoke-check.py")).read_text()
        self.assertIn("SIMCTL_CHILD_YEBISU_QA_AX='1'", driver)
        self.assertIn("idle_report(", driver)


class P9Timeout(unittest.TestCase):
    def test_xcodebuild_enforces_test_timeouts(self):
        command = p9.xcodebuild_test_command(Path("project"), "DEVICE", Path("derived"), Path("result.xcresult"))
        at = command.index("-test-timeouts-enabled")
        self.assertEqual(command[at + 1], "YES")
        self.assertIn("-only-testing:SmokeOracle/SmokeQA/testP9Core()", command)
        self.assertIn("id=DEVICE", command)

    def test_p9_core_has_an_execution_time_allowance(self):
        source = swift_source()
        setup = body(source, "override func setUpWithError(")
        match = re.search(r'name\.contains\("testP9Core"\)\s*\{\s*executionTimeAllowance = (\d+)', setup)
        self.assertIsNotNone(match)
        # testP9Core takes about 130 s on a hosted runner since L8; the allowance covers that
        # plus one 60 s animation wait (run 44276079121) and stays far under the job timeout.
        self.assertTrue(180 <= int(match.group(1)) <= 240, match.group(1))


class S3Identifiers(unittest.TestCase):
    """UI test redesign S3: P9, SH1-154 and P3 press QA identifiers and wait on qa.state."""
    TESTS = ("func testP9Core(", "func testSH154CoreImage(", "func testP3Terminal(")
    OCR = ("tapText(", "tapTextBox(", "tapMenuItem(", "textVisible(", "waitForRenderedText(", "tapRecorded(",
           "openSettings(in:", "openOverflowMenu(", "closeOverflowMenu(", "recognize(", "hideSoftwareKeyboard(")

    def setUp(self):
        self.source = swift_source()

    def test_no_ocr_coordinate_or_fixed_sleep(self):
        for signature in self.TESTS:
            function = "\n".join(statements(body(self.source, signature)))
            with self.subTest(test=signature):
                for call in self.OCR:
                    self.assertNotIn(call, function)
                self.assertNotRegex(function, r"\bsleep\(")
                self.assertNotIn("withNormalizedOffset", function)
                self.assertIn('launchEnvironment["YEBISU_QA_AX"] = "1"', function)
                self.assertIn("continueAfterFailure = false", function)

    def test_helpers_press_by_identifier(self):
        for signature in ("func openMenu(", "func menuItem(", "func typeIntoTerminal(", "func openSettingsSheet(",
                          "func setDensity("):
            function = body(self.source, signature)
            with self.subTest(helper=signature):
                self.assertIn("qa.press(", function)
                for call in self.OCR:
                    self.assertNotIn(call, function)

    def test_p9_sets_its_start_mode_at_launch(self):
        function = body(self.source, "func testP9Core(")
        self.assertIn('launchEnvironment["YEBISU_QA_START_SELECTION"] = "double-tap"', function)
        self.assertIn('start.bool("drag_select"), false', function)
        self.assertNotIn("Settings", "\n".join(line for line in statements(function)
                                                 if not line.startswith(("//", "app.launchEnvironment"))))

    def test_oracle_stages_are_unchanged(self):
        # p9-check and p3-check read these screenshots by name.
        for signature, stages in (("func testP9Core(", ['save("p9-split")', 'save("p9-reattached")']),
                                  ("func testSH154CoreImage(", ['save("sh154-core-image")']),
                                  ("func testP3Terminal(", ['save("p3-standard")', 'save("p3-compact")',
                                                            'save("p3-landscape")', 'save("p3-returned")'])):
            saved = [line for line in statements(body(self.source, signature)) if line.startswith("save(")]
            self.assertEqual(saved, stages, signature)

    def test_p9_runs_the_press_logic_checks(self):
        command = p9.xcodebuild_test_command(Path("project"), "DEVICE", Path("derived"), Path("result.xcresult"))
        self.assertIn("-only-testing:SmokeOracle/QAAxLogicTests", command)




class S6Identifiers(unittest.TestCase):
    """UI test redesign S6: P11, P14 and P15 press QA identifiers and wait on qa.state."""
    TESTS = ("func testP11Settings(", "private func observerProbe(", "func testP14ConnectedNotifications(",
             "func testP15Theme(")

    def setUp(self):
        self.source = swift_source()

    def test_no_ocr_or_window_point(self):
        for signature in self.TESTS:
            function = "\n".join(statements(body(self.source, signature)))
            with self.subTest(test=signature):
                for call in S3Identifiers.OCR:
                    self.assertNotIn(call, function)
                self.assertNotIn("withNormalizedOffset", function)
                self.assertNotIn("sleep(", function.replace("Thread.sleep(forTimeInterval: 0.2)", ""))
                self.assertIn('launchEnvironment["YEBISU_QA_AX"] = "1"', function)
                self.assertIn("continueAfterFailure = false", function)

    def test_the_old_presses_in_the_old_order(self):
        p11 = body(self.source, "func testP11Settings(")
        self.assertEqual(re.findall(r"qa\.press\((QAIds\.\w+)", p11), ["QAIds.p11Probe", "QAIds.settingsClose"])
        self.assertIn("try pressLarger(qa)", p11)
        larger = body(self.source, "func pressLarger(")
        self.assertIn("try qa.press(QAIds.fontLarger", larger)
        self.assertIn("self.fontSize($0) == size + 1", larger)
        p14 = body(self.source, "func testP14ConnectedNotifications(")
        self.assertEqual(re.findall(r"(?:qa\.press\(|p14Next\(qa, step: )(QAIds\.\w+|\d)", p14),
                         ["QAIds.terminalInput", "1", "QAIds.toast", "2", "3", "4", "5"])
        self.assertIn('XCTAssertLessThanOrEqual(frame.maxY, keyboardTop + 2, "toast is covered by the software keyboard")', p14)
        p15 = body(self.source, "func testP15Theme(")
        self.assertEqual(re.findall(r"tapAppearance\(qa", p15), ["tapAppearance(qa"] * 3)
        self.assertEqual(re.findall(r'typeIntoTerminal\(qa, app, "([^"]+)"\)', p15), ["P15-start", "P15-end"])
        self.assertIn('try pressBoardHost(qa, QAIds.boardHostBeta, host: "qa-beta")', p15)

    def test_vision_is_left_only_for_the_p15_host_text(self):
        """The owner decision keeps screenshot checks where the text is the subject: P15's host
        block. No other UI test imports Vision."""
        for path in sorted(TESTS.glob("*.swift")):
            with self.subTest(file=path.name):
                text = path.read_text()
                if path.name == "SmokeQA+Theme.swift":
                    self.assertEqual(text.count("VNRecognizeTextRequest()"), 1)
                    self.assertIn("VNRecognizeTextRequest()", body(text, "private func p15Host("))
                else:
                    self.assertNotIn("import Vision", text)
                    self.assertNotIn("VNRecognizeTextRequest", text)
        p12 = Path(__file__).with_name("p12-qa") / "Tests" / "P12QA.swift"
        self.assertNotIn("import Vision", p12.read_text())


if __name__ == "__main__":
    unittest.main()
