"""Check mutation-result classification; these are not Rust mutation executions."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "discard_mutations", Path(__file__).resolve().parents[1] / "check-discard-mutations.py")
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)

# Real rust-test output shape from the c137f38 new-composition failure.
# Test name/reason below are fixture inputs, not a claimed run of a new D1 test.
def failure(test, reason, code=101):
    return SimpleNamespace(returncode=code, stdout=f"""
thread 'module::{test}' (1) panicked at crates/yebisu/src/tests.rs:10:9:
{reason}
test module::{test} ... FAILED
failures:
    module::{test}
test result: FAILED. 0 passed; 1 failed; 0 ignored
""")


class Classification(unittest.TestCase):
    def test_expected_assertion_is_required(self):
        name = "clean-detach"
        test = "sh1_156_fresh_herdr1_accepts_printable_and_direct_text"
        self.assertTrue(M.behavioral_failure(name, test, failure(test, M.FAILURE_REASONS[name])))
        self.assertFalse(M.behavioral_failure(name, test, failure(test, "SH1-156 frame timeout")))
        self.assertFalse(M.behavioral_failure(name, test, failure(test, "wire: timed out waiting on channel")))
        self.assertFalse(M.behavioral_failure(name, test, failure(test, "unrelated panic")))

    def test_compile_failure_and_wrong_test_are_not_negatives(self):
        name = "host-discard"
        test = "sh1_156_host_switch_inherits_discarded_and_rejected_marks"
        reason = M.FAILURE_REASONS[name]
        self.assertFalse(M.behavioral_failure(name, test, failure("unrelated_test", reason)))
        self.assertFalse(M.behavioral_failure(name, test, failure(test, reason, code=1)))
        self.assertFalse(M.behavioral_failure(name, test, failure(test, reason + "\nerror[E0308]: mismatched types")))

    def test_reason_map_covers_each_mutation_once(self):
        names = [row[0] for row in M.MUTATIONS]
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(set(names), set(M.FAILURE_REASONS))

    def test_previous_real_failure_shape_is_recognized(self):
        test = "sh1_156_new_composition_after_discard_without_tap"
        with patch.dict(M.FAILURE_REASONS, {"historical": 'right: "new-composition"'}):
            result = failure(test, 'assertion `left == right` failed\n  left: ""\n right: "new-composition"')
            self.assertTrue(M.behavioral_failure("historical", test, result))


if __name__ == "__main__":
    unittest.main()
