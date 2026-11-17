"""smokeqa-labels-check.py: the checked-in SmokeQA is tied to the app, and each way of breaking the tie is caught."""
import contextlib
import importlib.util
import io
from pathlib import Path
import re
import shutil
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location("smokeqa_labels_check", HERE / "smokeqa-labels-check.py")
labels = importlib.util.module_from_spec(spec)
spec.loader.exec_module(labels)


def referenced_files():
    return sorted({path for table in (labels.SOURCES, labels.ALLOWED) for entry in table.values()
                   for path, _ in (entry if isinstance(entry, list) else entry[1])})


class Tree(unittest.TestCase):
    """A scratch repository: the SmokeQA tests and every app file the tables name, copied so a test can break one."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.tests = self.root / "Tests"
        shutil.copytree(HERE / "smoke-qa" / "Tests", self.tests)
        for path in referenced_files():
            (self.root / path).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, self.root / path)

    def run_check(self):
        rows, problems, call_sites = labels.check(self.root, self.tests)
        return rows, problems, call_sites

    def add_swift(self, body):
        (self.tests / "SmokeQA+Added.swift").write_text("extension SmokeQA {\n    func testAdded() throws {\n%s\n    }\n}\n" % body)

    def rewrite(self, path, old, new):
        file = self.root / path
        text = file.read_text()
        self.assertIn(old, text)
        file.write_text(text.replace(old, new))


class CheckedIn(Tree):
    def test_the_checked_in_tests_are_tied_to_the_checked_in_app(self):
        rows, problems, _ = labels.check(ROOT, HERE / "smoke-qa" / "Tests")
        self.assertEqual(problems, [])
        self.assertGreater(len(rows), 50)

    def test_a_scratch_copy_is_clean_too(self):
        self.assertEqual(self.run_check()[1], [])

    def test_every_call_site_is_classified_and_the_known_strings_are_found(self):
        source = labels.strip_comments("\n".join(p.read_text() for p in sorted(self.tests.glob("SmokeQA*.swift"))))
        found, problems = labels.collect(source)
        self.assertEqual(problems, [])
        for expected in ("Settings", "Observer next", "Native next", "High contrast", "paste-2.png", "HOSTS", "Host: ",
                         "qa-alpha", "idle", "Type here", "Theme", "Agent needs attention", "QuietHours"):
            with self.subTest(label=expected):
                self.assertIn(expected, found)

    def test_main_returns_zero_for_the_checked_in_tree_and_one_for_a_broken_one(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(labels.main(["--root", str(ROOT)]), 0)
            self.add_swift('        try tapText("Nonexistent control", in: app)')
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(labels.main(["--root", str(self.root), "--tests", str(self.tests)]), 1)


class AddedLabel(Tree):
    """A label SmokeQA looks for that no app source or allow-list entry names."""

    def assertCaught(self, body, label):
        self.add_swift(body)
        problems = self.run_check()[1]
        self.assertTrue(any(repr(label) in p and "may not show it" in p for p in problems), problems)

    def test_tap_text_of_a_text_the_app_does_not_show_fails(self):
        self.assertCaught('        try tapText("Nonexistent control", in: app)', "Nonexistent control")

    def test_every_other_way_to_name_a_label_fails_too(self):
        self.assertCaught('        _ = try tapTextBox("Ghost toast", in: app, attachAs: "x")', "Ghost toast")
        self.assertCaught('        XCTAssertTrue(textVisible("Ghost status"))', "Ghost status")
        self.assertCaught('        try waitForRenderedText("ghostword", what: "x")', "ghostword")
        self.assertCaught('        try tapText(flag ? "Ghost on" : "Ghost off", in: app)', "Ghost on")
        self.assertCaught('        let ghost = "Ghost constant"\n        try tapText(ghost, in: app)', "Ghost constant")
        self.assertCaught('        for (label, stage) in [("Ghost loop", "a"), ("Settings", "b")] {\n            try tapText(label, in: app)\n        }',
                          "Ghost loop")
        self.assertCaught('        XCTAssertTrue(text.contains("Ghost line"))', "Ghost line")

    def test_a_label_that_cannot_be_resolved_to_literals_fails(self):
        self.add_swift("        try tapText(computedLabel(), in: app)")
        problems = self.run_check()[1]
        self.assertTrue(any("cannot resolve the label argument" in p for p in problems), problems)

    def test_an_unknown_contains_receiver_fails(self):
        self.add_swift('        XCTAssertTrue(screenText.contains("Ghost line"))')
        problems = self.run_check()[1]
        self.assertTrue(any("screenText.contains" in p for p in problems), problems)

    def test_a_commented_out_call_is_not_a_label(self):
        self.add_swift('        // try tapText("Nonexistent control", in: app)')
        self.assertEqual(self.run_check()[1], [])

    def test_a_label_already_tied_adds_no_error(self):
        self.add_swift('        try tapText("Settings", in: app)')
        self.assertEqual(self.run_check()[1], [])


    def test_contains_on_an_expression_receiver_fails(self):
        for call in ('text.lowercased().contains("Ghost line")', 'text?.contains("Ghost line")'):
            with self.subTest(call=call):
                self.add_swift("        XCTAssertTrue(%s)" % call)
                problems = self.run_check()[1]
                self.assertTrue(any("Ghost line" in p and "contains" in p for p in problems), problems)

    def test_a_same_named_let_that_cannot_be_read_is_not_hidden_by_one_that_can(self):
        self.add_swift('        let ghost = "Close"\n        try tapText(ghost, in: app)')
        self.assertEqual(self.run_check()[1], [])
        (self.tests / "SmokeQA+Other.swift").write_text(
            "extension SmokeQA {\n    func testOther() throws {\n        let ghost = computedTitle()\n    }\n}\n")
        problems = self.run_check()[1]
        self.assertTrue(any("cannot resolve the label argument 'ghost'" in p for p in problems), problems)


class AppChanged(Tree):
    """The app stops painting what SmokeQA still taps."""

    def test_a_commented_out_control_fails(self):
        self.rewrite("crates/yebisu/src/terminal_view.rs", '"Reconnect"', '"Reconnect"')
        file = self.root / "crates/yebisu/src/terminal_view.rs"
        lines = file.read_text().splitlines()
        at = next(i for i, line in enumerate(lines) if '"Reconnect"' in line)
        lines[at] = "// " + lines[at]
        file.write_text("\n".join(lines) + "\n")
        problems = self.run_check()[1]
        self.assertTrue(any("'Reconnect'" in p and "no longer contains" in p for p in problems), problems)

    def test_a_block_commented_control_fails(self):
        file = self.root / "crates/yebisu/src/mock_peer.rs"
        text = file.read_text()
        self.assertIn("Type here", text)
        file.write_text("/* " + text.replace("*/", "* /") + " */\n")
        problems = self.run_check()[1]
        self.assertTrue(any("'Type here'" in p and "no longer contains" in p for p in problems), problems)

    def test_a_renamed_control_text_fails(self):
        self.rewrite("crates/yebisu/src/settings_sheet/view.rs", '"open-settings", "Settings"', '"open-settings", "Preferences"')
        problems = self.run_check()[1]
        self.assertTrue(any("'Settings'" in p and "no longer contains" in p for p in problems), problems)

    def test_a_deleted_source_file_fails(self):
        (self.root / "crates/yebisu/src/handoff_view.rs").unlink()
        problems = self.run_check()[1]
        self.assertTrue(any("'Paste image'" in p and "no longer contains" in p for p in problems), problems)


class AllowListSource(Tree):
    """An allow-list entry is only as good as the source that makes the text appear."""

    def test_the_source_of_an_allowed_text_disappearing_fails(self):
        (self.root / "crates/yebisu/src/core_mock/paste.rs").unlink()
        problems = self.run_check()[1]
        for name in ("paste-0.png", "paste-1.png", "paste-2.png"):
            with self.subTest(label=name):
                self.assertTrue(any(repr(name) in p and "no longer contains" in p and "allow-list reason" in p for p in problems), problems)

    def test_the_fragment_of_an_allowed_text_changing_fails(self):
        self.rewrite("crates/yebisu/src/app.rs", 'format!("Async counter: {}"', 'format!("Ticks: {}"')
        problems = self.run_check()[1]
        self.assertTrue(any("'Async counter'" in p and "no longer contains" in p for p in problems), problems)

    def test_the_font_metrics_changing_fails(self):
        self.rewrite("crates/yebisu/src/settings_pages/fonts.rs", "size_px: 14.0,", "size_px: 15.0,")
        problems = self.run_check()[1]
        self.assertTrue(any("'Standard 14/20'" in p and "no longer contains" in p for p in problems), problems)

    def test_an_allow_list_entry_no_test_uses_any_more_fails(self):
        with mock.patch.dict(labels.ALLOWED, {"Retired": ("no longer used", [("crates/yebisu/src/app.rs", "Async counter")])}):
            problems = self.run_check()[1]
        self.assertTrue(any("ALLOWED entry 'Retired' is no longer used" in p for p in problems), problems)

    def test_a_sources_entry_no_test_uses_any_more_fails(self):
        with mock.patch.dict(labels.SOURCES, {"Retired": [("crates/yebisu/src/app.rs", "Async counter")]}):
            problems = self.run_check()[1]
        self.assertTrue(any("SOURCES entry 'Retired' is no longer used" in p for p in problems), problems)

    def test_a_label_in_both_tables_fails(self):
        with mock.patch.dict(labels.ALLOWED, {"Settings": ("x", [("crates/yebisu/src/app.rs", "Async counter")])}):
            problems = self.run_check()[1]
        self.assertTrue(any("'Settings' is in both" in p for p in problems), problems)


class Tables(unittest.TestCase):
    def test_every_table_location_exists_in_the_checked_in_app(self):
        for path in referenced_files():
            with self.subTest(path=path):
                self.assertTrue((ROOT / path).is_file())

    def test_every_allow_list_entry_gives_a_reason(self):
        for label, (reason, locations) in labels.ALLOWED.items():
            with self.subTest(label=label):
                self.assertGreater(len(reason), 20)
                self.assertTrue(locations)


class Wiring(unittest.TestCase):
    def test_the_check_job_runs_it(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        check_job = re.search(r"^  check:\n(.*?)(?=^  [a-z][a-z-]*:\n|\Z)", workflow, re.S | re.M).group(1)
        self.assertIn("run: python3 scripts/ios/smokeqa-labels-check.py\n", check_job)


if __name__ == "__main__":
    unittest.main()
