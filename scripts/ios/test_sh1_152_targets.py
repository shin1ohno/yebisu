import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("sh1_152_targets", Path(__file__).with_name("sh1_152_targets.py"))
targets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(targets)


def log(overrides=None, passed=True):
    lines = []
    for name in (*targets.PASTE, *targets.NO_MENU, *targets.LONG_PRESS):
        record = {"name": name, "x": 1, "y": 2, "paste": name in targets.PASTE, "items": [],
                  "secondary": 1 if name in targets.PASTE else 0, "presented": 1 if name in targets.PASTE else 0}
        if name == "host-row-long-press":
            record["row_menu"] = True
        record.update((overrides or {}).get(name, {}))
        lines.append("t = 1.0s L61 TARGET " + json.dumps(record, sort_keys=True))
    if passed:
        lines.append(f"Test Case '-[SmokeOracle.EditMenuTargetsQA {targets.TEST}]' passed (40.1 seconds).")
    return "\n".join(lines)


class TargetsTests(unittest.TestCase):
    def test_paste_only_on_the_terminal_passes(self):
        verdict = targets.validate(log())
        self.assertEqual(set(verdict.values()), {"pass"})
        self.assertEqual(len(verdict), 11)

    def test_origin_main_paste_over_the_sidebar_fails_by_target(self):
        # The owner's report: Paste over an AGENTS row, as origin/main shows it.
        for name in targets.NO_MENU:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, f"{name}: Paste shown True, expected False"):
                targets.validate(log({name: {"paste": True, "items": ["Paste"]}}))

    def test_a_missing_terminal_menu_fails(self):
        with self.assertRaisesRegex(ValueError, "terminal: Paste shown False, expected True"):
            targets.validate(log({"terminal": {"paste": False}}))

    def test_the_app_receipts_name_a_missing_or_extra_presentation(self):
        with self.assertRaisesRegex(ValueError, r"terminal: the app presented no menu \(secondary clicks it saw: 0\)"):
            targets.validate(log({"terminal": {"presented": 0, "secondary": 0}}))
        with self.assertRaisesRegex(ValueError, "agents-row: the app presented its menu outside the terminal"):
            targets.validate(log({"agents-row": {"presented": 1}}))

    def test_a_closed_long_press_menu_fails(self):
        with self.assertRaisesRegex(ValueError, "the row's long-press menu did not open"):
            targets.validate(log({"host-row-long-press": {"row_menu": False}}))

    def test_missing_targets_and_failed_xctest_fail(self):
        text = "\n".join(line for line in log().splitlines() if '"status-bar"' not in line)
        with self.assertRaisesRegex(ValueError, "status-bar: not clicked"):
            targets.validate(text)
        with self.assertRaisesRegex(ValueError, "did not pass"):
            targets.validate(log(passed=False))

    def test_a_repeated_target_fails(self):
        with self.assertRaisesRegex(ValueError, "reported twice"):
            targets.targets(log() + "\n" + log().splitlines()[0])


if __name__ == "__main__":
    unittest.main()


class TraceTests(unittest.TestCase):
    """Issue #163: the app's edit menu traces are read from its record file, an unfinished last line
    is dropped, and other records are left out."""

    def test_only_edit_menu_traces_are_read(self):
        trace = {"kind": "edit-menu-trace", "token": 2, "after_ms": 300, "status": "Current",
                 "windows": [{"window": "app", "key": True, "menu_views": 5, "responder": "text_input"}]}
        data = (json.dumps({"kind": "heartbeat", "tick": 2}) + "\n" + json.dumps(trace) + "\n"
                + '{"kind": "edit-menu-trace", "tok').encode()
        self.assertEqual(targets.traces(data), [trace])

    def test_a_file_the_app_cut_short_still_yields_its_traces(self):
        trace = {"kind": "edit-menu-trace", "token": 1, "after_ms": 30, "status": "Current", "windows": []}
        data = (json.dumps(trace) + "\n" + json.dumps({"kind": "observation-limit", "written": 8192}) + "\n").encode()
        self.assertEqual(targets.traces(data), [trace])

    def test_an_unreadable_record_file_never_raises(self):
        source = Path(targets.__file__).read_text()
        keep = source[source.index("def keep_app_records("):source.index("def validate(")]
        self.assertIn("except (OSError, ValueError)", keep)

    def test_the_driver_keeps_the_records_after_the_test(self):
        source = Path(targets.__file__).read_text()
        main = source[source.index("def main():"):]
        self.assertLess(main.index('"terminate", device, "com.shin1ohno.yebisu"'), main.index("keep_app_records(device, directory)"))
        self.assertLess(main.index("keep_app_records(device, directory)"), main.index("verdict = validate(log)"))

