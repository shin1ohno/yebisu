"""p12-check.py hands P12QA its inputs; P12QA.swift and p12-oracle.py must agree on their names."""
import importlib.util
from pathlib import Path
import re
import unittest

HERE = Path(__file__).parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check = load("p12_check", "p12-check.py")
oracle = load("p12_oracle", "p12-oracle.py")
SWIFT = (HERE / "p12-qa/Tests/P12QA.swift").read_text()


class P12CheckEnvTests(unittest.TestCase):
    def test_documents_reach_the_runner_without_a_fault_by_default(self):
        env = check.xctest_env(Path("/sim/data/Documents"))
        self.assertEqual(env["TEST_RUNNER_P12_DOCUMENTS"], "/sim/data/Documents")
        self.assertNotIn("TEST_RUNNER_P12_QA_FAULT", env)

    def test_fault_reaches_the_runner_only_when_asked_for(self):
        env = check.xctest_env(Path("/d"), "drop-tap:remote")
        self.assertEqual(env["TEST_RUNNER_P12_QA_FAULT"], "drop-tap:remote")

    def test_swift_reads_the_names_the_prefix_strips_to(self):
        # xcodebuild strips TEST_RUNNER_, so the test reads the bare names.
        self.assertIn('environment["P12_DOCUMENTS"]', SWIFT)
        self.assertIn('environment["P12_QA_FAULT"]', SWIFT)
        self.assertNotIn('environment["TEST_RUNNER_', SWIFT)


class P12QASourceTests(unittest.TestCase):
    """P12QA.swift cannot run off a Mac; these pin the parts the oracle relies on."""

    def test_fixture_labels_are_the_oracle_steps(self):
        labels = re.search(r"fixtureControls = \[(.*?)\]", SWIFT, re.S).group(1)
        steps = {label.lower() for label in re.findall(r'"(\w+)"', labels)}
        self.assertEqual(steps, set(oracle.STEPS))

    def test_the_reveal_and_receipt_waits_are_not_shortened(self):
        self.assertIn("revealTimeout: TimeInterval = 10", SWIFT)
        self.assertIn("receiptTimeout: TimeInterval = 5", SWIFT)

    def test_a_reveal_is_repeated_once_and_the_assertion_after_it_is_kept(self):
        self.assertIn("if reveal == .completed || attempt == 2", SWIFT)
        self.assertIn("XCTAssertEqual(reveal, .completed", SWIFT)
        self.assertEqual(SWIFT.count('"kind": "reveal-retry"'), 1)
        self.assertEqual(SWIFT.count('"kind": "reveal-result"'), 1)

    def test_a_fixture_tap_is_repeated_once_and_must_then_be_received(self):
        self.assertEqual(SWIFT.count('"kind": "fixture-retry"'), 1)
        self.assertIn("XCTAssertTrue(received,", SWIFT)
        self.assertEqual(SWIFT.count("awaitSetupEnd(step, after: before)"), 2)

    def test_events_survive_a_failed_assertion(self):
        # A failed assertion skips `defer`; the events are attached in tearDown.
        self.assertIn("override func tearDown()", SWIFT)
        self.assertNotIn("defer {", SWIFT)


if __name__ == "__main__":
    unittest.main()
