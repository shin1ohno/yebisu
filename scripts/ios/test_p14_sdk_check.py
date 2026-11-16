"""Synthetic receipt-oracle tests; never run cargo, native code or mutations."""
import importlib.util
from pathlib import Path
import subprocess
import unittest

spec = importlib.util.spec_from_file_location("sdk_check", Path(__file__).with_name("p14-sdk-check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class NativeReceiptOracle(unittest.TestCase):
    def test_exact_inventory_and_ignored_count(self):
        check.require_pass(subprocess.CompletedProcess([], 0, "test result: ok. 13 passed; 0 failed; 1 ignored;", ""), 13, 1)
        for text in ["test result: ok. 3 passed; 0 failed; 1 ignored;", "test result: ok. 13 passed; 0 failed; 0 ignored;", ""]:
            with self.assertRaisesRegex(ValueError, "inventory"):
                check.require_pass(subprocess.CompletedProcess([], 0, text, ""), 13, 1)

    def test_only_intended_mutant_assertion_counts(self):
        test = check.RESPONSE_TEST
        assertion = "D5 six-second SDK response must reach consumer"
        check.require_mutant(subprocess.CompletedProcess([], 101, f"{test} ... FAILED", assertion), test, assertion)
        for code, out, err in [(101, "", "error[E0425]: compile failed"), (1, f"{test} ... FAILED", assertion), (101, f"{test} ... FAILED", "different panic"), (101, "other_test ... FAILED", assertion)]:
            with self.assertRaisesRegex(ValueError, "intended assertion"):
                check.require_mutant(subprocess.CompletedProcess([], code, out, err), test, assertion)

    def test_d5_mutation_restores_the_display_age_guard(self):
        sample = b"    if !responses.owns(tag, now) {\n        return false;\n    }"
        mutated = sample.replace(check.RESPONSE_NEEDLE, check.RESPONSE_MUTANT)
        self.assertIn(b"ticket.current()", mutated)
        self.assertIn(b"responses.owns(tag, now)", mutated)
        self.assertEqual(mutated.replace(check.RESPONSE_MUTANT, check.RESPONSE_NEEDLE), sample)
