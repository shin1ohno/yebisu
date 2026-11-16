"""Static guards for the state waits of SmokeQA.testInputAndLifecycleReceipts and the P9 timeout.

Swift cannot be compiled on the Linux CI host, so this reads SmokeQA.swift as text. The
failures it guards (class C of the L17 diagnosis) are a fixed sleep after Home, activate()
or typeText that lets the next step run before the app is ready, and a P9 hang that runs
into the 40-minute job timeout instead of failing the test."""
import importlib.util
from pathlib import Path
import re
import unittest

SWIFT = Path(__file__).with_name("smoke-qa") / "Tests" / "SmokeQA.swift"

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
        self.source = SWIFT.read_text()
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
        for signature in ("private func requireState(", "private func waitForRenderedText("):
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
        function = body(self.source, "private func requireState(")
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
        source = SWIFT.read_text()
        setup = body(source, "override func setUpWithError(")
        match = re.search(r'name\.contains\("testP9Core"\)\s*\{\s*executionTimeAllowance = (\d+)', setup)
        self.assertIsNotNone(match)
        # testP9Core takes about 60 s; the allowance must stay near twice that.
        self.assertTrue(90 <= int(match.group(1)) <= 180, match.group(1))


if __name__ == "__main__":
    unittest.main()
