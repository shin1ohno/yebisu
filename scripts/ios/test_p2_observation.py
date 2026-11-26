import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("p2", HERE / "p2-check.py")
p2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p2)
LINE = "restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa"
SHA256 = hashlib.sha256(LINE.encode()).hexdigest()


def receipt(sha256=SHA256, length=len(LINE)):
    return {"kind": "p2-pasteboard", "copied": 1, "pasteboard_sha256": sha256, "pasteboard_bytes": length}


class ObservationTests(unittest.TestCase):
    def records(self):
        records=[]
        for _ in range(2):
            records += [{"kind":"launch"},{"kind":"key-provider-preflight","phase":"startup","injected":True},{"kind":"key-provider-preflight","phase":"read","injected":True},{"kind":"setup","key_ready":True,"status":"Copied restricted public key."}]
        records.insert(4, receipt())
        return records
    def test_actual_nonsecret_setup_shape(self):
        self.assertEqual(p2.validate_records(self.records()), LINE)
    def test_negative_public_key_in_receipt(self):
        records=self.records();records[3]["key_line"]="FAKE_PUBLIC_KEY_SENTINEL"
        with self.assertRaisesRegex(RuntimeError,"public key leaked"):p2.validate_records(records)
    def test_negative_missing_second_launch_readiness(self):
        records=self.records();records[-1]["key_ready"]=False
        with self.assertRaisesRegex(RuntimeError,"not ready"):p2.validate_records(records)
    def test_negative_missing_copy(self):
        records=self.records();records=[{**r,"status":"ready"} if r.get("kind")=="setup" else r for r in records]
        with self.assertRaisesRegex(RuntimeError,"copy action"):p2.validate_records(records)
    def test_missing_preflight_fails_closed(self):
        records=[r for r in self.records() if r.get("kind")!="key-provider-preflight"]
        with self.assertRaisesRegex(RuntimeError,"preflight missing"):p2.validate_records(records)
    def test_non_injected_receipt_never_authorizes(self):
        with self.assertRaisesRegex(RuntimeError,"preflight missing"):
            p2.validate_preflight([{"kind":"key-provider-preflight","phase":"startup","injected":False}])
    def test_ready_before_read_preflight_fails(self):
        records=self.records();records[2],records[3]=records[3],records[2]
        with self.assertRaisesRegex(RuntimeError,"preceded provider"):p2.validate_records(records)
    def test_the_negative_controls_refuse_this_runs_own_receipts(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            p2.negative_controls(self.records(), LINE)
        self.assertEqual(out.getvalue().count("P2 negative control refused:"), 4)


class PasteboardTests(unittest.TestCase):
    """The app's read-back SHA-256 is the pasteboard check on hosted and Mac mini (owner decision
    ui-test-redesign-decision.json); simctl pbpaste is a Mac mini aid (exit 60 on hosted, run 44151000668)."""

    def test_the_expected_digest_is_the_key_lines_utf8_sha256(self):
        self.assertEqual(p2.expected_pasteboard(), (SHA256, len(LINE.encode())))
        self.assertEqual(p2.KEY_LINE, LINE)

    def test_the_app_reading_back_the_key_line_passes(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(p2.validate_pasteboard([receipt()], LINE), 1)

    def test_negative_another_digest_fails(self):
        with self.assertRaisesRegex(RuntimeError, "differs"):
            p2.validate_pasteboard([receipt(sha256=hashlib.sha256(b"x").hexdigest())], LINE)

    def test_negative_the_right_digest_with_another_length_fails(self):
        with self.assertRaisesRegex(RuntimeError, "differs"):
            p2.validate_pasteboard([receipt(length=len(LINE) + 1)], LINE)

    def test_negative_a_later_wrong_copy_fails_too(self):
        with self.assertRaisesRegex(RuntimeError, "differs"):
            p2.validate_pasteboard([receipt(), receipt(sha256="0" * 64)], LINE)

    def test_negative_no_text_read_back_fails_by_name(self):
        with self.assertRaisesRegex(RuntimeError, "read no text back"):
            p2.validate_pasteboard([receipt(sha256=None, length=None)], LINE)

    def test_negative_no_receipt_fails_by_name(self):
        with self.assertRaisesRegex(RuntimeError, "no pasteboard read-back receipt"):
            p2.validate_pasteboard([{"kind": "setup"}], LINE)

    def test_pbpaste_retries_then_reports_none(self):
        calls = []
        def fails(*args, **kwargs):
            calls.append(args[0])
            raise p2.subprocess.CalledProcessError(60, args[0])
        self.assertIsNone(p2.pbpaste("dev", check_output=fails, sleep=lambda _: None))
        self.assertEqual(len(calls), 3)

    def test_hosted_runner_never_reads_pbpaste(self):
        reads = []
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertFalse(p2.pasteboard_aid("dev", LINE, hosted=True, read=lambda d: reads.append(d) or "x"))
        self.assertEqual(reads, [])
        self.assertIn("job 134279069207", out.getvalue())

    def test_off_the_hosted_runner_the_exact_line_passes_the_aid(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(p2.pasteboard_aid("dev", LINE, hosted=False, read=lambda d: LINE))

    def test_a_failed_pbpaste_read_does_not_fail_the_aid(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertFalse(p2.pasteboard_aid("dev", LINE, hosted=False, read=lambda d: None))
        self.assertIn("could not read", out.getvalue())

    def test_negative_other_text_fails_the_aid(self):
        with self.assertRaisesRegex(RuntimeError, "differs"):
            p2.pasteboard_aid("dev", LINE, hosted=False, read=lambda d: "something else")

    def test_main_passes_the_hosted_flag_from_github_actions(self):
        text = (HERE / "p2-check.py").read_text()
        self.assertIn('pasteboard_aid(device, expected_line, os.environ.get("GITHUB_ACTIONS") == "true")', text)
        self.assertIn("validate_pasteboard(records, expected_line)", text)

    def test_xctest_gets_the_expected_digest(self):
        command, environment = p2.xcodebuild_test_command(Path("/spec"), "dev", Path("/dd"), Path("/r"))
        self.assertIn("-only-testing:SmokeOracle/SmokeQA/testP2KeysAndRestart", command)
        self.assertEqual(environment, {"TEST_RUNNER_P2_PASTEBOARD_SHA256": SHA256,
                                       "TEST_RUNNER_P2_PASTEBOARD_BYTES": str(len(LINE))})


class AfterTerminateTests(unittest.TestCase):
    """p2-setup.jsonl is read once, after the app was terminated (runs 44155672313, 44236636053 and
    44288317467 failed reading it while the app ran)."""

    def write(self, data):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "p2-setup.jsonl"
        path.write_bytes(data)
        return path

    def lines(self, records, time=10):
        return "".join(json.dumps({**r, "time": time}) + "\n" for r in records).encode()

    def test_a_complete_file_is_read_whole(self):
        records = ObservationTests.records(None)
        self.assertEqual(len(p2.read_after_terminate(self.write(self.lines(records)), 0)), len(records))

    def test_records_of_an_earlier_run_are_left_out(self):
        path = self.write(self.lines([{"kind": "launch"}], time=1) + self.lines(ObservationTests.records(None), time=10))
        self.assertEqual(p2.read_after_terminate(path, 5)[0]["time"], 10)
        self.assertEqual(len(p2.read_after_terminate(path, 5)), len(ObservationTests.records(None)))

    def test_negative_a_partly_written_line_fails_by_name(self):
        path = self.write(self.lines(ObservationTests.records(None)) + b'{"kind":"set')
        with self.assertRaisesRegex(RuntimeError, "partly written line after the app was terminated"):
            p2.read_after_terminate(path, 0)

    def test_negative_a_malformed_complete_line_raises(self):
        path = self.write(self.lines(ObservationTests.records(None)) + b"{not json}\n")
        with self.assertRaises(json.JSONDecodeError):
            p2.read_after_terminate(path, 0)

    def test_negative_a_missing_file_fails_by_name_without_waiting(self):
        path = self.write(b"")
        path.unlink()
        with self.assertRaisesRegex(RuntimeError, "does not exist after the app was terminated"):
            p2.read_after_terminate(path, 0)


class TerminateTests(unittest.TestCase):
    def runner(self, code):
        calls = []
        def run(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, code, "", "" if code == 0 else "simctl said no")
        return run, calls

    def test_a_running_app_is_terminated(self):
        run, calls = self.runner(0)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(p2.terminate("dev", runner=run))
        self.assertEqual(calls, [["xcrun", "simctl", "terminate", "dev", "com.shin1ohno.yebisu"]])

    def test_an_app_that_already_exited_is_not_an_error(self):
        run, _ = self.runner(3)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertFalse(p2.terminate("dev", runner=run))

    def test_negative_any_other_failure_stops_before_the_read(self):
        run, _ = self.runner(1)
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(RuntimeError, "could not terminate"):
            p2.terminate("dev", runner=run)


class StructureTests(unittest.TestCase):
    """The driver never reads the app's files while the app runs (UI test redesign S2)."""
    TEXT = (HERE / "p2-check.py").read_text()
    MAIN = TEXT[TEXT.index("def main():"):]
    SWIFT = (HERE / "smoke-qa/Tests/SmokeQA+Keys.swift").read_text()

    def test_no_launch_before_xctest_and_no_live_read(self):
        self.assertNotIn("sim.sh", self.TEXT)
        self.assertNotIn("--wait", self.TEXT)
        self.assertNotIn("live_jsonl", self.TEXT)
        self.assertNotIn("SIMCTL_CHILD_", self.TEXT)
        self.assertNotIn("read_until_valid", self.TEXT)

    def test_the_only_read_of_the_receipts_follows_the_terminate(self):
        self.assertEqual(self.MAIN.count("read_after_terminate("), 1)
        self.assertEqual(self.MAIN.count("terminate(device)"), 1)
        self.assertLess(self.MAIN.index("run(*command"), self.MAIN.index("terminate(device)"))
        self.assertLess(self.MAIN.index("terminate(device)"), self.MAIN.index("read_after_terminate("))
        self.assertLess(self.MAIN.index("terminate(device)"), self.MAIN.index("shutil.copyfile(receipts"))

    def test_the_swift_test_waits_for_automatic_load_and_confirms_copy(self):
        for unwanted in ("tapText", "textVisible", "waitForRenderedText", "recognize(", "sleep(",
                         "qa.keys.read"):
            self.assertNotIn(unwanted, self.SWIFT)
        for expected in ('qa.keys.provider', 'qa.keys.copy', '"YEBISU_QA_AX": "1"',
                         'string("sheet") == "add-host"', 'bool("key_ready") == true',
                         'P2_PASTEBOARD_SHA256', 'int("copied") == 1'):
            self.assertIn(expected, self.SWIFT)
        self.assertEqual(re.findall(r"qa\.press\(([^,]+),", self.SWIFT), ["QAAxIDs.keysCopy"])
        self.assertEqual(len(re.findall(r"try p2Launch\(qa,", self.SWIFT)), 2)
        self.assertEqual(len(re.findall(r"\bapp\.launch\(\)", self.SWIFT)), 2,
                         "XCTest launches the app both times")


if __name__ == "__main__":unittest.main()


class BlackRestartEvidence(unittest.TestCase):
    """Issue #197: the restarted window's second picture and the app's frame receipts are evidence;
    the verdict stays the first picture's."""

    def test_attachments_are_matched_by_exact_name(self):
        manifest = [{"attachments": [
            {"suggestedHumanReadableName": "p2-restarted_0_A.png", "exportedFileName": "a.png"},
            {"suggestedHumanReadableName": "p2-restarted-later_0_B.png", "exportedFileName": "b.png"},
        ]}]
        self.assertEqual([a["exportedFileName"] for a in p2.attachments_named(manifest, "p2-restarted")], ["a.png"])
        self.assertEqual([a["exportedFileName"] for a in p2.attachments_named(manifest, "p2-restarted-later")], ["b.png"])

    def test_restart_frames_come_after_the_last_launch_only(self):
        records = [
            {"kind": "launch", "pid": 1},
            {"kind": "input-receipt", "receipt": "frame", "outcome": "presented", "previous": "none", "previous_count": 0},
            {"kind": "launch", "pid": 2},
            {"kind": "lifecycle", "phase": "Active", "generation": 0},
            {"kind": "lifecycle", "phase": "Inactive", "generation": 0},
            {"kind": "input-receipt", "receipt": "touch", "stage": "began"},
            {"kind": "input-receipt", "receipt": "frame", "outcome": "zero_size", "previous": "none", "previous_count": 0},
        ]
        self.assertEqual(p2.restart_frames(records), [
            {"kind": "lifecycle", "phase": "Active"},
            {"kind": "lifecycle", "phase": "Inactive"},
            {"kind": "input-receipt", "outcome": "zero_size", "previous": "none", "previous_count": 0},
        ])

    def test_a_black_restart_is_explained_then_still_fails(self):
        text = (HERE / "p2-check.py").read_text()
        main = text[text.index("def main():"):]
        check = main.index('str(destination / "screenshot.png"), "101419", "--min", "0.4")')
        handler = main[check:main.index("expected_line = validate_records(records)")]
        self.assertIn("except subprocess.CalledProcessError:", handler)
        self.assertLess(handler.index("explain_black_restart(records, destination)"), handler.index("raise"))
        swift = (HERE / "smoke-qa/Tests/SmokeQA+Keys.swift").read_text()
        self.assertLess(swift.index('save("p2-restarted")'), swift.index('save("p2-restarted-later")'))
