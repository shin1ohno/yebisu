"""qa-ax-ids-check.py: the checked-in Swift tests and app agree on QA identifiers, and each way of breaking that is caught."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location("qa_ax_ids_check", HERE / "qa-ax-ids-check.py")
ids = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ids)

APP = "crates/yebisu/src/app.rs"
MARK = "crate::qa_ax::mark(crate::qa_ax::ids::P0_TAP, P0_TAP_LABEL)"
HELPERS = "scripts/ios/smoke-qa/Tests/QAAx.swift"


class CheckedIn(unittest.TestCase):
    def test_the_checked_in_tests_are_tied_to_the_checked_in_app(self):
        rows, problems = ids.check(ROOT)
        self.assertEqual(problems, [])
        by_id = {row[0]: row for row in rows}
        self.assertIn("qa.state", by_id)
        self.assertIn("qa.p0.tap", by_id)
        self.assertIn("app.rs", by_id["qa.p0.tap"][2], "P0_TAP is marked in the app root")
        self.assertIn("QAAxSelfTest.swift", by_id["qa.p0.tap"][3], "the self-test presses it")


class Mutations(unittest.TestCase):
    """A scratch copy of the app sources and the Swift tests; each test breaks one tie."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        shutil.copytree(ROOT / ids.SRC, self.root / ids.SRC)
        for path in ROOT.glob(ids.SWIFT_GLOB):
            target = self.root / path.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)

    def problems(self):
        return ids.check(self.root)[1]

    def rewrite(self, rel, old, new):
        path = self.root / rel
        text = path.read_text()
        self.assertIn(old, text)
        path.write_text(text.replace(old, new, 1))

    def add_swift(self, body):
        (self.root / "scripts/ios/smoke-qa/Tests/QAAxAdded.swift").write_text(
            "import XCTest\nfinal class Added: XCTestCase {\n    func testAdded() throws {\n%s\n    }\n}\n" % body)

    def assertProblem(self, *fragments):
        problems = self.problems()
        self.assertTrue(any(all(f in p for f in fragments) for p in problems), problems)

    def test_a_scratch_copy_is_clean(self):
        self.assertEqual(self.problems(), [])

    def test_an_identifier_swift_uses_that_the_app_does_not_define(self):
        self.add_swift('        try QAAx(app).press("qa.p0.tapp")')
        self.assertProblem("'qa.p0.tapp'", "QAAxAdded.swift:4", "does not define it")

    def test_a_mark_removed_from_the_app(self):
        # The negative control of plan 3.4: unmark P0_TAP and the identifier the self-test uses is named.
        self.rewrite(APP, MARK, "None::<gpui::AnyElement>")
        self.assertProblem("P0_TAP", "'qa.p0.tap'", "no qa_ax::mark call uses it")

    def test_a_mark_with_a_literal_or_unknown_name(self):
        self.rewrite(APP, MARK, 'crate::qa_ax::mark("qa.p0.tap", P0_TAP_LABEL)')
        problems = self.problems()
        self.assertTrue(any("spells 'qa.p0.tap'" in p and "app.rs" in p for p in problems), problems)

    def test_a_mark_naming_a_constant_that_does_not_exist(self):
        self.rewrite(APP, MARK, "crate::qa_ax::mark(crate::qa_ax::ids::P0_TAPP, P0_TAP_LABEL)")
        self.assertProblem("qa_ax::mark(crate::qa_ax::ids::P0_TAPP", "does not name a constant")

    def test_two_constants_with_one_identifier(self):
        self.rewrite(ids.IDS, 'P0_TAP: &str = "qa.p0.tap";',
                     'P0_TAP: &str = "qa.p0.tap";\npub(crate) const P0_AGAIN: &str = "qa.p0.tap";')
        self.assertProblem("P0_TAP and P0_AGAIN both define 'qa.p0.tap'")

    def test_a_constant_that_is_not_an_identifier(self):
        self.rewrite(ids.IDS, '"qa.p0.tap"', '"p0 tap"')
        self.assertProblem("P0_TAP = 'p0 tap' is not a QA identifier")

    def test_a_commented_out_mark_does_not_count(self):
        self.rewrite(APP, ".children(%s)" % MARK, "// .children(%s)" % MARK)
        self.assertProblem("P0_TAP", "no qa_ax::mark call uses it")

    def test_a_composed_identifier_cannot_be_checked(self):
        self.add_swift('        let name = "tap"\n        _ = QAAx(app).element("qa.p0.\\(name)")')
        self.assertProblem("composes an identifier at run time")

    def test_helpers_never_decide_by_is_hittable(self):
        self.rewrite(HELPERS, "let element = try find(id, timeout: timeout)",
                     "let element = try find(id, timeout: timeout)\n        if !element.isHittable { return before }")
        self.assertProblem("QAAx.swift uses isHittable")

    def test_the_uikit_half_widened_beyond_the_simulator(self):
        self.rewrite(ids.QA_AX, '#[cfg(all(target_os = "ios", target_abi = "sim"))]\n#[path = "qa_ax_native.rs"]',
                     '#[cfg(target_os = "ios")]\n#[path = "qa_ax_native.rs"]')
        self.assertProblem("no longer declares native")

    def test_the_build_gate_widened(self):
        self.rewrite(ids.QA_AX, 'cfg!(all(target_os = "ios", target_abi = "sim"))', 'cfg!(target_os = "ios")')
        self.assertProblem("no longer declares BUILD")

    def test_another_module_declaring_the_uikit_half(self):
        self.rewrite("crates/yebisu/src/main.rs", "mod qa_ax;", "mod qa_ax;\n#[path = \"qa_ax_native.rs\"]\nmod leak;")
        self.assertProblem("main.rs names qa_ax_native")

    def test_a_missing_ids_file(self):
        (self.root / ids.IDS).unlink()
        self.assertProblem("qa_ax_ids.rs is missing")


if __name__ == "__main__":
    unittest.main()
