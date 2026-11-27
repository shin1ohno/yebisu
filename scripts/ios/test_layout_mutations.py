"""Every layout source mutation still has its anchor (#244).

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_layout_mutations.py'

layout-mutations.py replaces one exact source fragment per case. When a
feature PR rewrites that fragment, the case can no longer run; the CI step
reached it only after the 31 cases before it (43 min, #244), and #234's own
run hit the job timeout first. This runs on every `just test`, in a second,
so the PR that rewrites a mutated line fails here. Pure file reads; no build.
"""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


def load():
    spec = importlib.util.spec_from_file_location("layout_mutations", HERE / "layout-mutations.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class LayoutMutationAnchors(unittest.TestCase):
    def setUp(self):
        self.module = load()

    def test_every_anchor_occurs_exactly_once(self):
        self.assertEqual(self.module.stale_anchors(), [])

    def test_every_mutation_changes_its_source(self):
        for name, _, old, new, _ in self.module.CASES:
            with self.subTest(name):
                self.assertNotEqual(old, new)

    def test_case_names_are_unique(self):
        names = [case[0] for case in self.module.CASES]
        self.assertEqual(len(names), len(set(names)))

    def test_a_rewritten_anchor_is_reported(self):
        # Negative control: the check must name a case whose fragment is gone
        # (the #244 shape) and one whose fragment now occurs twice.
        name, filename, old, new, test = self.module.CASES[0]
        source = (self.module.SRC / filename).read_text()
        twice = next(line for line in source.splitlines() if source.count(line) > 1 and line.strip())
        self.module.CASES = [("gone", filename, old + "\n// not in the source", new, test),
                             ("twice", filename, twice, new, test), (name, filename, old, new, test)]
        self.assertEqual(self.module.stale_anchors(),
                         [f"gone: source anchor must occur exactly once in {filename}",
                          f"twice: source anchor must occur exactly once in {filename}"])

    def test_main_refuses_before_any_build(self):
        # main() must stop on a stale anchor before it runs the first baseline.
        self.module.CASES = [("gone", "layout.rs", "// not in the source", "", "layout::tests::x")]
        self.module.run = lambda *_: self.fail("a baseline ran despite a stale anchor")
        with self.assertRaises(SystemExit) as raised:
            self.module.main()
        self.assertIn("gone: source anchor must occur exactly once in layout.rs", str(raised.exception))


class LayoutMutationOrder(unittest.TestCase):
    """main() runs every baseline on the unmodified source first (issue #257),
    then each mutation, and keeps both gates. `run` is faked; no build."""

    def setUp(self):
        self.module = load()
        self.temporary = tempfile.TemporaryDirectory()
        root = Path(self.temporary.name)
        self.module.SRC = root / "src"
        self.module.OUT = root / "out"
        self.module.SRC.mkdir()
        self.originals = {"a.rs": "fn a() { 1 }\n", "b.rs": "fn b() { 2 }\n"}
        for filename, text in self.originals.items():
            (self.module.SRC / filename).write_text(text)
        self.module.CASES = [("first", "a.rs", "{ 1 }", "{ 0 }", "a::tests::one"),
                             ("second", "b.rs", "{ 2 }", "{ 0 }", "b::tests::two")]
        self.calls = []
        self.fail_baseline = None
        self.survivor = None

    def tearDown(self):
        self.temporary.cleanup()

    def sources(self):
        return {name: (self.module.SRC / name).read_text() for name in self.originals}

    def fake_run(self, name, test):
        self.calls.append((name, self.sources()))
        if name.endswith("-baseline"):
            if name == self.fail_baseline:
                return 101, f"test {test} ... FAILED\n0 passed; 1 failed", 0.1
            return 0, f"test {test} ... ok\n1 passed; 0 failed", 0.1
        if name == self.survivor:
            return 0, f"test {test} ... ok\n1 passed; 0 failed", 0.2
        module = test.split("::tests::")[0]
        return 101, f"test {test} ... FAILED\npanicked at crates/yebisu/src/{module}/tests/x.rs:1:1", 0.2

    def test_every_baseline_runs_on_the_unmodified_source_before_any_mutation(self):
        self.module.run = self.fake_run
        self.module.main()
        self.assertEqual([name for name, _ in self.calls],
                         ["first-baseline", "second-baseline", "first", "second"])
        for name, sources in self.calls[:2]:
            with self.subTest(name):
                self.assertEqual(sources, self.originals)
        self.assertEqual(self.calls[2][1], {"a.rs": "fn a() { 0 }\n", "b.rs": "fn b() { 2 }\n"})
        self.assertEqual(self.calls[3][1], {"a.rs": "fn a() { 1 }\n", "b.rs": "fn b() { 0 }\n"})
        self.assertEqual(self.sources(), self.originals)
        verdict = json.loads((self.module.OUT / "verdict.json").read_text())
        self.assertEqual([(case["mutation"], case["baseline_seconds"], case["mutation_seconds"])
                          for case in verdict["cases"]], [("first", 0.1, 0.2), ("second", 0.1, 0.2)])

    def test_a_failing_baseline_stops_before_any_mutation(self):
        # Negative control for the baseline gate: the second test fails on the
        # original source, so no source is mutated and no mutation runs.
        self.module.run = self.fake_run
        self.fail_baseline = "second-baseline"
        with self.assertRaises(SystemExit) as raised:
            self.module.main()
        self.assertEqual(str(raised.exception), "second: baseline did not run and pass the named regression")
        self.assertEqual([name for name, _ in self.calls], ["first-baseline", "second-baseline"])
        self.assertEqual(self.sources(), self.originals)

    def test_a_surviving_mutation_still_fails_and_restores_the_source(self):
        # Negative control for the mutation gate.
        self.module.run = self.fake_run
        self.survivor = "second"
        with self.assertRaises(SystemExit) as raised:
            self.module.main()
        self.assertEqual(str(raised.exception), "second: intended assertion failure not observed")
        self.assertEqual(self.sources(), self.originals)


if __name__ == "__main__":
    unittest.main()
