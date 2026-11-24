"""JSONL that a running app may still be writing: only complete lines are records."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("live_jsonl", Path(__file__).with_name("live_jsonl.py"))
live_jsonl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(live_jsonl)

FIXTURE = Path(__file__).parent / "fixtures/app-unterminated-tail.jsonl"


class LiveJsonlTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def write(self, data, name="live.jsonl"):
        path = self.root / name
        path.write_bytes(data)
        return path

    def test_recorded_85_byte_fragment_after_valid_lines_is_dropped(self):
        # app.jsonl of run 43992978378 job 133738049997: one complete paint line, then 85 bytes.
        data = FIXTURE.read_bytes()
        complete, fragment = data.rsplit(b"\n", 1)
        self.assertEqual(len(fragment), 85)
        self.assertNotEqual(fragment[-1:], b"\n")
        with self.assertRaises(json.JSONDecodeError):
            json.loads(fragment)
        records = live_jsonl.read_records(self.write(data))
        self.assertEqual(records, [json.loads(complete)])
        self.assertEqual(records[0]["kind"], "paint")

    def test_fragment_after_many_lines_keeps_every_complete_line(self):
        lines = [json.dumps({"n": n}) for n in range(50)]
        data = ("\n".join(lines) + "\n").encode() + FIXTURE.read_bytes().rsplit(b"\n", 1)[1]
        self.assertEqual(live_jsonl.read_records(self.write(data)), [{"n": n} for n in range(50)])

    def test_fragment_that_stops_inside_a_multibyte_character_is_not_decoded(self):
        data = b'{"n":1}\n{"title":"' + "·".encode()[:1]
        self.assertEqual(live_jsonl.read_records(self.write(data)), [{"n": 1}])
        self.assertEqual(live_jsonl.count_lines(self.write(data)), 1)

    def test_fragment_that_happens_to_be_valid_json_is_still_dropped(self):
        # `{"n":2}` without its newline may be the first half of a longer record; only a newline completes it.
        self.assertEqual(live_jsonl.read_records(self.write(b'{"n":1}\n{"n":2}')), [{"n": 1}])

    def test_malformed_line_in_the_middle_still_fails(self):
        path = self.write(b'{"n":1}\n{"n":\n{"n":3}\n')
        with self.assertRaises(json.JSONDecodeError):
            live_jsonl.read_records(path)
        with self.assertRaises(json.JSONDecodeError):
            live_jsonl.read_records(self.write(b'{"n":1}\n{"n":\n{"n":3}\n{"n":4', "tail.jsonl"))

    def test_blank_complete_line_is_malformed_as_before(self):
        with self.assertRaises(json.JSONDecodeError):
            live_jsonl.read_records(self.write(b'{"n":1}\n\n{"n":3}\n'))

    def test_empty_file_and_trailing_newline_behave_as_before(self):
        empty = self.write(b"", "empty.jsonl")
        self.assertEqual(live_jsonl.read_records(empty), [])
        self.assertEqual(live_jsonl.count_lines(empty), 0)
        terminated = self.write(b'{"n":1}\n{"n":2}\n', "terminated.jsonl")
        self.assertEqual(live_jsonl.read_records(terminated), [{"n": 1}, {"n": 2}])
        self.assertEqual(live_jsonl.count_lines(terminated), 2)
        # Same answers as the former splitlines() reader.
        self.assertEqual(live_jsonl.read_records(terminated), [json.loads(l) for l in terminated.read_text().splitlines()])

    def test_only_a_fragment_is_no_records(self):
        path = self.write(b'{"n":')
        self.assertEqual(live_jsonl.read_records(path), [])
        self.assertEqual(live_jsonl.count_lines(path), 0)

    def test_count_matches_parsed_lines(self):
        data = FIXTURE.read_bytes()
        path = self.write(data)
        self.assertEqual(live_jsonl.count_lines(path), len(live_jsonl.read_records(path)))
        self.assertEqual(live_jsonl.count_lines(path), 1)
        self.assertEqual(len(data.splitlines()), 2, "the former count would include the fragment")
        self.assertEqual(live_jsonl.count_lines(self.root / "missing.jsonl"), 0)

    def test_offset_taken_while_the_app_is_writing_selects_only_later_records(self):
        # selection-settings-check.py: previous = count_lines(...) before the run, read_records(path, previous) after.
        path = self.write(b'{"n":0}\n{"n":1}\n{"n":2')
        previous = live_jsonl.count_lines(path)
        self.assertEqual(previous, 2)
        path.write_bytes(path.read_bytes() + b'}\n{"n":3}\n{"n":4')
        self.assertEqual(live_jsonl.read_records(path, previous), [{"n": 2}, {"n": 3}])
        self.assertEqual(live_jsonl.read_records(path, previous)[0], live_jsonl.read_records(path)[previous])


class GapTests(unittest.TestCase):
    """The app writes observation-dropped (and observation-limit) in place of records it lost
    (crates/yebisu/src/observation.rs); a file holding one is incomplete and must not pass."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.path = Path(temporary.name) / "app.jsonl"

    def test_a_dropped_record_fails_the_read(self):
        self.path.write_bytes(b'{"kind":"launch"}\n{"kind":"observation-dropped","count":3,"time":1.0}\n{"kind":"paint"}\n')
        with self.assertRaisesRegex(ValueError, "observation gap.*observation-dropped"):
            live_jsonl.read_records(self.path)
        with self.assertRaisesRegex(ValueError, "observation gap"):
            live_jsonl.parse(self.path.read_bytes())

    def test_the_writer_limit_fails_the_read(self):
        self.path.write_bytes(b'{"kind":"paint"}\n{"kind":"observation-limit","written":8192}\n')
        with self.assertRaisesRegex(ValueError, "observation-limit"):
            live_jsonl.read_records(self.path)

    def test_a_gap_before_the_offset_is_not_this_run(self):
        self.path.write_bytes(b'{"kind":"observation-dropped","count":1}\n{"kind":"paint"}\n')
        self.assertEqual(live_jsonl.read_records(self.path, 1), [{"kind": "paint"}])

    def test_complete_records_pass_unchanged(self):
        records = [{"kind": "settings-lifecycle", "phase": "Inactive"}, {"kind": "paint"}]
        self.assertIs(live_jsonl.reject_gaps(records), records)



def beats(pid, times, end=None):
    """A launch record, heartbeats at times, and optionally one last record at end."""
    records = [{"kind": "launch", "pid": pid, "time": times[0] - 1}]
    records += [{"kind": "heartbeat", "tick": 2 * (n + 1), "time": t} for n, t in enumerate(times)]
    if end is not None:
        records.append({"kind": "input-receipt", "receipt": "touch", "time": end})
    return records


class HeartbeatGapTests(unittest.TestCase):
    """Issue #142 stage 2: a stalled main thread leaves a hole in the app's 2 s heartbeat."""

    def test_regular_heartbeats_leave_no_gap(self):
        self.assertEqual(live_jsonl.heartbeat_gaps(beats(7, [10 + 2 * n for n in range(50)], end=109)), [])

    def test_a_25_6_second_hole_is_one_gap(self):
        times = [10, 12, 14, 39.6, 41.6]
        self.assertEqual(live_jsonl.heartbeat_gaps(beats(7, times, end=42)),
                         [{"from": 14, "to": 39.6, "seconds": 25.6, "pid": 7}])

    def test_up_to_two_and_a_half_intervals_is_no_gap(self):
        self.assertEqual(live_jsonl.heartbeat_gaps(beats(7, [10, 15, 20])), [])
        self.assertEqual(len(live_jsonl.heartbeat_gaps(beats(7, [10, 15.1]))), 1)

    def test_gaps_are_split_by_launch(self):
        # The first launch's last heartbeat and the next launch's first one are 30 s apart:
        # that is a relaunch, not a stall.
        records = beats(7, [10, 12, 14], end=14.5) + beats(8, [44, 46, 60])
        self.assertEqual(live_jsonl.heartbeat_gaps(records), [{"from": 46, "to": 60, "seconds": 14, "pid": 8}])

    def test_a_trailing_hole_after_the_last_heartbeat_is_a_gap(self):
        self.assertEqual(live_jsonl.heartbeat_gaps(beats(7, [10, 12], end=72)),
                         [{"from": 12, "to": 72, "seconds": 60, "pid": 7, "trailing": True}])

    def test_a_file_without_heartbeats_has_no_gaps(self):
        self.assertEqual(live_jsonl.heartbeat_gaps([{"kind": "launch", "pid": 1, "time": 1}, {"kind": "x", "time": 90}]), [])


if __name__ == "__main__":
    unittest.main()
