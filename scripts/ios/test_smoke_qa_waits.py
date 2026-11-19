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
    def setUp(self):
        self.source = swift_source()
        self.test = statements(body(self.source, "func testInputAndLifecycleReceipts("))

    def following(self, prefix):
        """Statements that directly follow each statement starting with prefix."""
        return [self.test[at + 1] for at, line in enumerate(self.test) if line.startswith(prefix) and at + 1 < len(self.test)]

    def test_no_fixed_sleep_follows_home_activate_or_portrait_typing(self):
        for prefix in ("XCUIDevice.shared.press(.home)", "app.activate()", 'app.typeText("P0-input")',
                       'app.typeText("-returned")', 'app.typeText("-idle")'):
            with self.subTest(prefix=prefix):
                steps = self.following(prefix)
                self.assertTrue(steps, prefix + " not found")
                for step in steps:
                    self.assertNotIn("sleep(", step)

    def test_each_wait_is_a_named_state_wait(self):
        text = "\n".join(self.test)
        self.assertIn("press(.home)\n" + 'try requireState(app, [.runningBackground, .runningBackgroundSuspended], "app left the foreground after press(.home)")', text)
        self.assertIn('app.activate()\ntry requireState(app, [.runningForeground], "activate() after press(.home)")', text)
        for word in ("input", "returned", "idle"):
            self.assertIn(f'try waitForRenderedText("{word}", what:', text)

    def test_a_failed_wait_stops_the_test_at_once(self):
        self.assertIn("continueAfterFailure = false", "\n".join(self.test))
        for signature in ("func requireState(", "func waitForRenderedText("):
            with self.subTest(signature=signature):
                function = body(self.source, signature)
                self.assertIn("XCTFail(", function)
                self.assertIn("throw NSError(", function)
                self.assertIn("timeout", function)

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
        self.assertIn("sleep(60)", self.test)
        self.assertIn("XCTAssertEqual(app.state, .runningForeground)", self.test)


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


if __name__ == "__main__":
    unittest.main()
