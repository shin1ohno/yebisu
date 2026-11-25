"""resend_budget.py: one job's proven resends and their limit (issue #142 stage 4, Owner decision 5)."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).parent
spec = importlib.util.spec_from_file_location("resend_budget", HERE / "resend_budget.py")
budget = importlib.util.module_from_spec(spec)
spec.loader.exec_module(budget)


def line(kind, id_="qa.p12.remote"):
    return "QA-RESEND " + json.dumps({"kind": kind, "id": id_, "reason": "proven lost", "time": 1.0})


class ParseTests(unittest.TestCase):
    def test_parse_reads_only_resend_lines(self):
        text = "\n".join(["Test Case started", line("press-retry"), "QAAX press qa.x confirmed",
                          "    t =  3.0s " + line("home-retry", "P2 after Home"), "QA-RESENDING is not a mark"])
        self.assertEqual([r["kind"] for r in budget.parse(text)], ["press-retry", "home-retry"])
        self.assertEqual(budget.parse(""), [])
        self.assertEqual(budget.parse(None), [])

    def test_a_broken_resend_line_still_counts(self):
        self.assertEqual(budget.parse("QA-RESEND {not json"), [{"kind": "unparsed", "line": "QA-RESEND {not json"}])


class JudgeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.ledger = Path(temporary.name) / "target/sim-resends.jsonl"

    def ledger_with(self, *counts):
        for index, count in enumerate(counts):
            budget.append(self.ledger, f"check-{index}", [{"kind": "press-retry", "id": f"qa.{n}"} for n in range(count)])

    def test_up_to_the_limit_is_green_and_the_total_is_printed(self):
        for counts, total in (((0,), 0), ((1, 0), 1), ((1, 2, 0), 3)):
            with self.subTest(counts=counts):
                self.ledger.unlink(missing_ok=True)
                self.ledger_with(*counts)
                code, message = budget.judge(self.ledger)
                self.assertEqual(code, 0, message)
                self.assertIn(f"Simulator input resends in this job: {total} (limit 3)", message)

    def test_more_than_the_limit_fails_as_simulator_input_unstable(self):
        self.ledger_with(2, 0, 2)
        code, message = budget.judge(self.ledger)
        self.assertEqual(code, 1)
        self.assertTrue(message.startswith("Simulator input unstable: 4 proven resends in this job (limit 3): "), message)
        self.assertIn("check-2: press-retry qa.1", message)

    def test_a_missing_or_empty_ledger_fails(self):
        code, message = budget.judge(self.ledger)
        self.assertEqual(code, 1)
        self.assertIn("no resend ledger", message)
        self.ledger.parent.mkdir(parents=True)
        self.ledger.write_text("")
        self.assertEqual(budget.judge(self.ledger)[0], 1)

    def test_a_check_with_no_resends_still_writes_its_line(self):
        resends = budget.record("sh1-152-check", "no resend here\n", ledger=self.ledger)
        self.assertEqual(resends, [])
        rows = [json.loads(row) for row in self.ledger.read_text().splitlines()]
        self.assertEqual(rows, [{"check": "sh1-152-check", "count": 0, "resends": []}])

    def test_the_command_line_exit_code_is_the_verdict(self):
        self.ledger_with(4)
        script = HERE / "resend_budget.py"
        result = subprocess.run([sys.executable, "-I", str(script), "judge", str(self.ledger)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("Simulator input unstable: 4 proven resends", result.stdout)

    def test_run_tee_streams_and_keeps_the_output(self):
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            code, output = budget.run_tee([sys.executable, "-c", "print('a'); print('" + line("pick-retry") + "')"])
        self.assertEqual(code, 0)
        self.assertEqual(output, printed.getvalue())
        self.assertEqual([r["kind"] for r in budget.parse(output)], ["pick-retry"])


if __name__ == "__main__":
    unittest.main()
