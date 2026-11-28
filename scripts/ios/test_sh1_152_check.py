"""Negative controls for independent failure inputs and exact wire events."""
import importlib.util
import copy
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("sh1_152_oracle", Path(__file__).with_name("sh1_152_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class MenuOracleTests(unittest.TestCase):
    def fixture(self):
        records, messages, wire = [], [], []
        for case in oracle.BASIC_CASES:
            copying = case.startswith("E2")
            payload = f"sh1-152-clip-{case}-0"
            records.extend([
                {"case": case, "kind": "sh1-152-capture", "count": 2, "time": 1, "destination": 'Pane("t3")' if case.endswith("core") else 'Pane("w1:p1")'},
                {"case": case, "kind": "edit-menu-exec", "result": "executed", "action": "Copy" if copying else "Paste", "time": 2},
                {"case": case, "kind": "sh1-152-result", "clipboard": "日本語" if copying else payload, "time": 3},
            ])
            values = [f"sh1-152-start-{case}-0", *([] if copying else [payload]), f"sh1-152-end-{case}-0"]
            if case.endswith("core"):
                if not copying:
                    records.append({"case": case, "kind": "terminal-input", "variant": "Paste", "terminal": "t3"})
                wire.extend({"terminal": "t3", "bytes": list(b"\x1b[200~" + value.encode() + b"\x1b[201~")} for value in values)
            else:
                messages.extend({"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Paste": value}]}} for value in values)
        return records, messages, wire

    def test_real_shape_positive_and_same_text_key_wrong_pane_outside_interval_negatives(self):
        fixture = self.fixture()
        self.assertIn("PASS", oracle.validate_basic(*fixture))
        for mutation in ["key", "pane", "outside", "duplicate", "clipboard", "core-key", "core-pane", "core-byte", "core-menu-destination"]:
            records, messages, wire = copy.deepcopy(fixture)
            if mutation == "key":
                messages[1]["ClientShellPaneInput"]["events"] = [{"Key": {"code": {"Char": "sh1-152-clip-E1-text-0"}}}]
            elif mutation == "pane":
                messages[1]["ClientShellPaneInput"]["pane_id"] = "w1:p2"
            elif mutation == "outside":
                row = next(r for r in records if r.get("kind") == "edit-menu-exec")
                row["time"] = 4
            elif mutation == "duplicate":
                records.append(next(r for r in records if r.get("kind") == "edit-menu-exec"))
            elif mutation == "clipboard":
                next(r for r in records if r.get("kind") == "sh1-152-result")["clipboard"] = "wrong"
            elif mutation == "core-key":
                next(r for r in records if r.get("kind") == "terminal-input")["variant"] = "Key"
            elif mutation == "core-menu-destination":
                next(r for r in records if r.get("case") == "E1-core" and r.get("kind") == "sh1-152-capture")["destination"] = 'Pane("t1")'
            elif mutation == "core-pane":
                wire[1]["terminal"] = "core-B"
            else:
                wire[1]["bytes"] = list(b"sh1-152-clip-E1-core-0")
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                oracle.validate_basic(records, messages, wire)

    def test_failure_input_current_is_fail_not_unverified(self):
        for acquire, drawing, text in [([False], True, False), ([True], False, False)]:
            row = {"dropped": 0, "status": "Current", "samples": [{"restart_reached": True, "acquire_returns": acquire, "drawing_owned": drawing, "text_owned": text}]}
            with self.assertRaisesRegex(ValueError, "produced switch failure"):
                oracle.check_switch(row)
            row["status"] = "Revoked(SwitchLeftNoResponder)"
            self.assertEqual(oracle.check_switch(row), "pass")

    def test_successful_acquire_and_missing_restart_are_only_unverified(self):
        row = {"dropped": 0, "status": "Current", "samples": [{"restart_reached": True, "acquire_returns": [True], "drawing_owned": False, "text_owned": True}]}
        self.assertIn("unverified", oracle.check_switch(row))
        row["samples"][0].update(restart_reached=False, acquire_returns=[False])
        self.assertIn("unverified", oracle.check_switch(row))
        row["dropped"] = 1
        with self.assertRaisesRegex(ValueError, "dropped"):
            oracle.check_switch(row)

    def test_intervals_refuse_outside_input_and_duplicate_boundaries(self):
        def row(text):
            return {"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"Paste": text}]}}
        start, end = row("sh1-152-start-E1-text-0"), row("sh1-152-end-E1-text-0")
        valid = [start, row("payload"), end]
        self.assertEqual(oracle.segments(valid)[("E1-text", 0)]["events"], [("w1:p1", {"Paste": "payload"})])
        for invalid in [[row("outside"), *valid], [start, start, end], [start], [end]]:
            with self.assertRaises(ValueError):
                oracle.segments(invalid)

    def test_visible_menu_shows_only_usable_items(self):
        cases = ["E1-text", "E2-text", "E3c"]
        log = "\n".join([
            "SH1-152 VISIBLE E1-text selected=false labels=Paste",
            "SH1-152 VISIBLE E2-text selected=true labels=Copy,Paste",
            "SH1-152 VISIBLE E3c selected=true labels=Paste,Copy",
            "SH1-152 VISIBLE E3c selected=false labels=Paste",
        ])
        self.assertIn("PASS", oracle.visible_menus(log, cases))
        for bad, expected in [
            (log.replace("E1-text selected=false labels=Paste", "E1-text selected=false labels=Copy,Paste"), "usable-items"),
            (log.replace("E2-text selected=true labels=Copy,Paste", "E2-text selected=true labels=Paste"), "usable-items"),
            (log.replace("E2-text selected=true", "E2-text selected=false").replace("labels=Copy,Paste\nSH1-152 VISIBLE E3c selected=true", "labels=Paste\nSH1-152 VISIBLE E3c selected=true"), "first menu selection"),
            ("\n".join(log.splitlines()[1:]), "not recorded"),
        ]:
            with self.subTest(expected=expected), self.assertRaisesRegex(ValueError, expected):
                oracle.visible_menus(bad, cases)

    def test_view_kind_accepts_actual_uikit_class_names(self):
        self.assertEqual(oracle.view_kind("herdr_uikit::window::text_input::view_class::GPUITextInputView0.1.0"), "text")
        self.assertEqual(oracle.view_kind("HerdrUIKitGPUIView"), "drawing")
        self.assertEqual(oracle.view_kind("UITextField"), "UITextField")


class DriverTests(unittest.TestCase):
    def test_the_test_runner_reads_no_receipt_file(self):
        # UI test redesign S4: EditMenuQA waits on qa.state and reads none of the app's files while
        # the app runs; the driver no longer hands it the Documents path and reads the files itself
        # after terminating the app (test_terminate_before_read.py).
        driver = (Path(__file__).with_name("sh1-152-check.py")).read_text()
        self.assertNotIn("TEST_RUNNER_SH1_152_DOCUMENTS", driver)
        swift = (Path(__file__).parent / "smoke-qa/Tests/EditMenuQA.swift").read_text()
        for reader in ("SH1_152_DOCUMENTS", "contentsOfDirectory", "String(contentsOf", "p4-peer.jsonl\")", "FileManager"):
            self.assertNotIn(reader, swift)


check_spec = importlib.util.spec_from_file_location("sh1_152_check", Path(__file__).with_name("sh1-152-check.py"))
check = importlib.util.module_from_spec(check_spec)
check_spec.loader.exec_module(check)


class CheckReportTests(unittest.TestCase):
    """Issue #142 stage 4: what sh1-152-check.py adds to its verdict and to the job's resend ledger."""

    def test_the_verdict_names_each_case_files_heartbeat_gaps(self):
        import json, tempfile
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            rows = [{"kind": "launch", "pid": 7, "time": 0}, *({"kind": "heartbeat", "tick": 2 * n, "time": t} for n, t in enumerate([2, 4, 30.5, 32]))]
            (directory / "sh1-152-E4-text-0.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
            (directory / "sh1-152-E1-key-0.jsonl").write_text(json.dumps({"kind": "observation-dropped", "count": 1, "time": 1}) + "\n")
            report = check.heartbeat_report(directory)
        self.assertEqual(report[0]["file"], "sh1-152-E1-key-0.jsonl")
        self.assertIn("unreadable", report[0])
        self.assertEqual(report[1], {"from": 4, "to": 30.5, "seconds": 26.5, "pid": 7, "file": "sh1-152-E4-text-0.jsonl"})

    def test_a_launch_retry_needs_a_launch_that_started_no_process(self):
        resend = 'QA-RESEND {"id": "E2-text", "kind": "launch-retry", "reason": "not running", "time": 1.0}'
        attempts = "SH1-152 LAUNCH E1-text 4.2\nSH1-152 LAUNCH E2-text 108.0\n" + resend + "\nSH1-152 LAUNCH E2-text 5.1\n"
        self.assertEqual(check.launch_retries_proven("SH1-152 LAUNCH E1-text 4.2\n", ""), "no launch-retry")
        two_starts = "app-stderr launch pid=11\nx\napp-stderr launch pid=12\n"
        self.assertIn("1 launch-retry proven: 2 app starts for 3 attempts", check.launch_retries_proven(attempts, two_starts))
        with self.assertRaisesRegex(ValueError, "launch-retry not proven: 3 app starts for 3 launch attempts"):
            check.launch_retries_proven(attempts, two_starts + "app-stderr launch pid=13\n")

    def test_a_no_pid_launch_retry_needs_the_failed_launch_to_have_started_the_app(self):
        """Issue #247: the no-pid launch ran the app (qa.state was read), so both attempts start a
        process; a no-pid retry with one start fewer is not proven, nor is a not-started retry with
        the start the no-pid kind would have."""
        no_pid = ('QA-RESEND {"id": "E3c", "kind": "launch-retry", "reason": "no-pid-app-running: XCTest has no pid '
                  'for the launched app while its qa.state was read: Application \'com.shin1ohno.yebisu\' does not have '
                  'a process ID", "time": 1.0}')
        attempts = "SH1-152 LAUNCH E3b 4.2\nSH1-152 LAUNCH E3c 134.7\n" + no_pid + "\nSH1-152 LAUNCH E3c 5.1\n"
        three = "app-stderr launch pid=11\napp-stderr launch pid=12\napp-stderr launch pid=13\n"
        self.assertEqual(check.launch_retries_proven(attempts, three),
                         "1 launch-retry proven: 3 app starts for 3 attempts (1 no-pid-app-running)")
        with self.assertRaisesRegex(ValueError, "launch-retry not proven: 2 app starts for 3 launch attempts, 0 retries "
                                                "of a launch that started nothing and 1 no-pid-app-running retries"):
            check.launch_retries_proven(attempts, "app-stderr launch pid=11\napp-stderr launch pid=12\n")
        not_started = attempts.replace('"reason": "no-pid-app-running: ', '"reason": "the launch failed: ')
        with self.assertRaisesRegex(ValueError, "launch-retry not proven: 3 app starts"):
            check.launch_retries_proven(not_started, three)

    def test_the_check_writes_its_ledger_line_before_any_verdict_and_also_on_failure(self):
        source = (Path(__file__).with_name("sh1-152-check.py")).read_text()
        main = source[source.index("def main("):]
        run = main.index('result = sim_ready.run_test(xctest_command := ["xcodebuild", "test"')
        record = main.index('\n        resend_budget.record("sh1-152-check", result.stdout)\n')
        self.assertRegex(main, r'\n        resend_budget\.record\("sh1-152-check", result\.stdout\)\n', "an unconditional statement")
        self.assertLess(run, record)
        self.assertLess(record, main.index("finally:"), "recorded before terminate, whatever XCTest's exit code")
        self.assertLess(record, main.index('raise SystemExit("SH1-152 FAIL'))
        self.assertIn('verdicts["heartbeat_gaps"] = heartbeat_report(directory)', main)


if __name__ == "__main__":
    unittest.main()
