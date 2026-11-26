"""Every layout source mutation still has its anchor (#244).

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_layout_mutations.py'

layout-mutations.py replaces one exact source fragment per case. When a
feature PR rewrites that fragment, the case can no longer run; the CI step
reached it only after the 31 cases before it (43 min, #244), and #234's own
run hit the job timeout first. This runs on every `just test`, in a second,
so the PR that rewrites a mutated line fails here. Pure file reads; no build.
"""

import importlib.util
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
