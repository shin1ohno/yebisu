"""qa-ax-selftest.py and QAAxSelfTest.swift agree, and the self-test runs only when asked.

Swift cannot be compiled on the Linux CI host, so this reads the Swift files as text."""
import importlib.util
from pathlib import Path
import re
import unittest

HERE = Path(__file__).resolve().parent
TESTS = HERE / "smoke-qa" / "Tests"
spec = importlib.util.spec_from_file_location("qa_ax_selftest", HERE / "qa-ax-selftest.py")
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)
SELFTEST = (TESTS / "QAAxSelfTest.swift").read_text()
HELPERS = (TESTS / "QAAx.swift").read_text()


def code(swift):
    """The Swift text without `//` comment lines (the doc comments name what the code avoids)."""
    return "\n".join(line for line in swift.splitlines() if not line.strip().startswith("//"))


class SelfTest(unittest.TestCase):
    def test_the_driver_names_the_swift_test(self):
        target, cls, method = driver.TEST.split("/")
        self.assertEqual(target, "SmokeOracle")
        self.assertIn("final class %s: XCTestCase" % cls, SELFTEST)
        self.assertIn("func %s() throws" % method, SELFTEST)
        self.assertEqual(driver.PASSED, "%s]' passed" % method)

    def test_it_skips_unless_the_driver_asks(self):
        # xcodebuild passes TEST_RUNNER_<NAME> to the runner as <NAME>.
        body = SELFTEST[SELFTEST.index("func testQAAxSelfTest"):]
        first = next(line.strip() for line in body.splitlines()[1:] if line.strip())
        self.assertEqual(first, 'try XCTSkipUnless(ProcessInfo.processInfo.environment["QA_AX_SELFTEST"] != nil, '
                                '"S1 self-test only")')
        self.assertIn('"TEST_RUNNER_QA_AX_SELFTEST": "1"', (HERE / "qa-ax-selftest.py").read_text())

    def test_no_other_driver_runs_it(self):
        for path in HERE.glob("*.py"):
            if path.name == "qa-ax-selftest.py" or path.name.startswith("test_"):
                continue
            self.assertFalse("QAAxSelfTest" in path.read_text(), path.name)

    def test_the_app_is_launched_with_the_layer_on(self):
        self.assertIn('"YEBISU_QA_AX": "1"', SELFTEST)

    def test_waits_read_the_state_directly(self):
        # S0: XCTNSPredicateExpectation took 1.4-1.5 s per wait; a direct read about 25 ms.
        for name, source in (("QAAxSelfTest.swift", SELFTEST), ("QAAx.swift", HELPERS)):
            self.assertFalse("XCTNSPredicateExpectation" in code(source), name)
        self.assertRegex(HELPERS, re.compile(r"func waitState\(.*timeout: TimeInterval = 10"))
        self.assertIn("throw QAAxError.timeout(what: what", HELPERS)


if __name__ == "__main__":
    unittest.main()
