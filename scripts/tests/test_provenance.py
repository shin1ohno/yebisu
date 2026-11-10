"""Provenance coverage and production Git-history regression tests."""

import csv
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
from provenance_fixture import history

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / ("scripts/" + name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load("check-provenance")
triage = load("triage-upstream")
UPSTREAM = "https://github.com/penso/herdr-gpui"


def write_rows(root, rows):
    with (root / "PROVENANCE.tsv").open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=checker.FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def row(path, base):
    return dict(kind="model", destination="model.rs", source_repository="https://github.com/shin1ohno/herdr-gpui",
                source_commit=base, source_path=path, modified="yes", upstream_repository=UPSTREAM,
                upstream_ref="main", upstream_path=path, last_triaged=base, last_triaged_at="2026-11-10")


class ProvenanceTests(unittest.TestCase):
    def test_repository_registers_copy_and_model_sources(self):
        errors, rows = checker.validate(ROOT)
        self.assertEqual(errors, [])
        self.assertEqual({r["kind"] for r in rows}, {"copy", "model"})
        self.assertEqual({r["upstream_repository"] for r in rows}, {UPSTREAM})

    def test_missing_model_and_unregistered_crate_copy_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            profile = root / "scripts/ios/profile.py"
            profile.parent.mkdir(parents=True)
            profile.write_text("pass\n")
            copy = root / "crates/yebisu/src/upstream/terminal_painter.rs"
            copy.parent.mkdir(parents=True)
            copy.write_text("// unregistered copy\n")
            registered = row("scripts/ios/profile.py", "1" * 40)
            registered.update(kind="copy", destination="scripts/ios/profile.py")
            write_rows(root, [registered])
            subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            errors, _ = checker.validate(root)
            self.assertIn("PROVENANCE.tsv must contain at least one model row", errors)
            expected = "unregistered copy: crates/yebisu/src/upstream/terminal_painter.rs"
            self.assertIn(expected, errors)
            # Positive control: registering the tracked copy removes its error.
            entry = row("terminal_painter.rs", "1" * 40)
            entry.update(kind="copy", destination=str(copy.relative_to(root)))
            write_rows(root, [registered, entry])
            self.assertNotIn(expected, checker.validate(root)[0])
            print("D2 negative: " + expected)

    def test_full_diff_rename_and_post_move_change_are_followed(self):
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary) / "fork"
            data, commits = history(repo)
            current, events = triage.inspect_row(repo, row(data["old_path"], commits["base"]), commits["9c32a9270"])
            self.assertEqual(current, data["new_path"])
            self.assertEqual([event[0] for event in events], [commits["8c1719924"], commits["9c32a9270"]])
            self.assertEqual(events[1][2:5], (data["new_path"], data["new_path"], False))
            self.assertFalse(events[0][4])
            print("D4 real-blob history: 8c1719924 R100 -> 9c32a9270 M at inherited " + current)

    def test_default_checkout_detects_registered_terminal_change(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "fixture"
            data, commits = history(repo)
            (root / "docs").mkdir()
            (root / "docs/upstream-log.md").write_text("")
            original = row(data["terminal_path"], commits["9c32a9270"])
            write_rows(root, [original])
            calls = []

            def local_remote(*args):
                calls.append(args)
                # Exercise the default clone/fetch path against the offline
                # projection; only transport is substituted, never inspection.
                if args[1] == "clone":
                    self.assertEqual(args[-2], UPSTREAM)
                    self.assertEqual(args[-3], "main")
                    subprocess.run([*args[:-2], str(repo), args[-1]], check=True)
                else:
                    subprocess.run(args, check=True)

            with patch.object(triage, "ROOT", root), patch.dict(os.environ, {}, clear=True), patch.object(triage, "guarded", local_remote):
                self.assertEqual(triage.main(), 0)
                self.assertEqual(triage.main(), 0)  # existing-cache fetch, no duplicates
            self.assertEqual(len(calls), 2)
            log = (root / "docs/upstream-log.md").read_text()
            self.assertIn("3a1b61753a1138965b8191189eca6b9e670ebcc7", log)
            self.assertIn(data["terminal_path"], log)
            self.assertEqual(log.count("decision: pending review"), 2)
            with (root / "PROVENANCE.tsv").open() as stream:
                updated = next(csv.DictReader(stream, delimiter="\t"))
            self.assertEqual(updated["last_triaged"], commits["head"])
            self.assertEqual(updated["source_commit"], original["source_commit"])
            self.assertEqual(updated["source_path"], original["source_path"])
            print("D1 default upstream scan: registered terminal.rs change 3a1b61753 found")

    def test_backward_position_refuses_without_updating_ledger_or_log(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "fixture"
            data, commits = history(repo)
            write_rows(root, [row(data["terminal_path"], commits["3a1b61753"])])
            before = (root / "PROVENANCE.tsv").read_bytes()
            with patch.object(triage, "ROOT", root), patch.object(triage, "source_checkout", return_value=repo):
                subprocess.run(["git", "-C", str(repo), "update-ref", "refs/remotes/origin/main", commits["base"]], check=True)
                with self.assertRaisesRegex(ValueError, "refusing backwards or divergent"):
                    triage.main()
            self.assertEqual((root / "PROVENANCE.tsv").read_bytes(), before)
            self.assertFalse((root / "docs/upstream-log.md").exists())
            print("D1 negative: refusing backwards or divergent triage position; ledger unchanged")


if __name__ == "__main__":
    unittest.main()
