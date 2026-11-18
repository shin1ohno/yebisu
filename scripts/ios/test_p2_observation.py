import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
spec = importlib.util.spec_from_file_location("p2", Path(__file__).with_name("p2-check.py"))
p2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p2)

class ObservationTests(unittest.TestCase):
    def records(self):
        records=[]
        for _ in range(2):
            records += [{"kind":"launch"},{"kind":"key-provider-preflight","phase":"startup","injected":True},{"kind":"key-provider-preflight","phase":"read","injected":True},{"kind":"setup","key_ready":True,"status":"Copied restricted public key."}]
        return records
    def test_actual_nonsecret_setup_shape(self):
        self.assertEqual(p2.validate_records(self.records()), "restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa")
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


class PasteboardTests(unittest.TestCase):
    """Layout-smoke diagnosis D3-1: simctl pbpaste exits 60 on GitHub-hosted runners."""
    LINE = "restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa"

    def test_pbpaste_retries_then_reports_none(self):
        calls = []
        def fails(*args, **kwargs):
            calls.append(args[0])
            raise p2.subprocess.CalledProcessError(60, args[0])
        self.assertIsNone(p2.pbpaste("dev", check_output=fails, sleep=lambda _: None))
        self.assertEqual(len(calls), 3)

    def test_hosted_runner_skips_only_the_pasteboard_read_with_the_reason(self):
        reads = []
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertFalse(p2.verify_pasteboard("dev", self.LINE, hosted=True, read=lambda d: reads.append(d) or "x"))
        self.assertEqual(reads, [], "the hosted runner never reads the receiving pasteboard")
        self.assertIn("P2 SKIPPED the receiving pasteboard read", out.getvalue())
        self.assertIn("job 134279069207", out.getvalue())
        self.assertIn("job 134611018720", out.getvalue())

    def test_off_the_hosted_runner_the_exact_line_passes(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(p2.verify_pasteboard("dev", self.LINE, hosted=False, read=lambda d: self.LINE))

    def test_negative_a_wrong_pasteboard_fails_off_the_hosted_runner(self):
        with self.assertRaisesRegex(RuntimeError, "differs"):
            p2.verify_pasteboard("dev", self.LINE, hosted=False, read=lambda d: "something else")

    def test_negative_a_timed_out_read_fails_off_the_hosted_runner(self):
        def times_out(*args, **kwargs):
            raise p2.subprocess.TimeoutExpired(args[0], 30)
        read = lambda device: p2.pbpaste(device, check_output=times_out, sleep=lambda _: None)
        with self.assertRaisesRegex(RuntimeError, "could not be read"):
            p2.verify_pasteboard("dev", self.LINE, hosted=False, read=read)

    def test_main_passes_the_hosted_flag_from_github_actions(self):
        text = (Path(__file__).with_name("p2-check.py")).read_text()
        self.assertIn('verify_pasteboard(device, expected_line, os.environ.get("GITHUB_ACTIONS") == "true")', text)
        self.assertNotIn("check_pasteboard(pbpaste(", text)


class LiveReceiptTests(unittest.TestCase):
    """Layout-smoke diagnosis D3-1b: p2-setup.jsonl is read while the app may still append to it."""

    def write(self, text):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "p2-setup.jsonl"
        path.write_text(text)
        return path

    def lines(self, records):
        return "".join(json.dumps({**r, "time": 10}) + "\n" for r in records)

    def test_a_partly_written_last_line_is_not_an_error(self):
        records = ObservationTests.records(None)
        path = self.write(self.lines(records) + '{"kind":"set')
        got, line = p2.read_until_valid(path, 0, p2.validate_records, sleep=lambda _: None)
        self.assertEqual(len(got), len(records))
        self.assertTrue(line.startswith("restrict ssh-ed25519"))

    def test_negative_a_malformed_complete_line_still_raises(self):
        path = self.write(self.lines(ObservationTests.records(None)) + "{not json}\n")
        with self.assertRaises(json.JSONDecodeError):
            p2.read_until_valid(path, 0, p2.validate_records, sleep=lambda _: None)

    def test_a_copy_receipt_that_lands_on_a_later_read_is_waited_for(self):
        records = ObservationTests.records(None)
        path = self.write(self.lines(records[:-1]))
        waits = []
        def land(_):
            waits.append(1)
            path.write_text(self.lines(records))
        got, _ = p2.read_until_valid(path, 0, p2.validate_records, sleep=land)
        self.assertEqual(len(waits), 1)
        self.assertEqual(len(got), len(records))

    def test_a_file_the_app_creates_late_is_waited_for(self):
        records = ObservationTests.records(None)
        path = self.write(self.lines(records))
        text = path.read_text()
        path.unlink()
        waits = []
        def land(_):
            waits.append(1)
            path.write_text(text)
        got, _ = p2.read_until_valid(path, 0, p2.validate_records, sleep=land)
        self.assertEqual(len(waits), 1)
        self.assertEqual(len(got), len(records))

    def test_negative_a_file_that_never_appears_fails(self):
        path = self.write("")
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            p2.read_until_valid(path, 0, p2.validate_records, attempts=2, sleep=lambda _: None)

    def test_negative_a_copy_receipt_that_never_lands_fails(self):
        records = ObservationTests.records(None)
        path = self.write(self.lines([{**r, "status": "ready"} if r.get("kind") == "setup" else r for r in records]))
        with self.assertRaisesRegex(RuntimeError, "copy action"):
            p2.read_until_valid(path, 0, p2.validate_records, attempts=2, sleep=lambda _: None)


if __name__ == "__main__":unittest.main()
