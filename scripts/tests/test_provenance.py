"""Provenance coverage and source-path migration regression tests."""

import csv
import importlib.util
import subprocess
import tempfile
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_provenance", ROOT / "scripts/check-provenance.py"
)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class ProvenanceTests(unittest.TestCase):
    def test_repository_registers_copy_and_model_sources(self):
        errors, rows = checker.validate(ROOT)
        self.assertEqual(errors, [])
        self.assertEqual({row["kind"] for row in rows}, {"copy", "model"})

    def test_missing_model_row_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "scripts/ios").mkdir(parents=True)
            (root / "scripts/ios/profile.py").write_text("pass\n")
            (root / "copy.txt").write_text("copy\n")
            (root / "model.txt").write_text("model\n")
            digest = "1" * 40
            with (root / "PROVENANCE.tsv").open("w", newline="") as stream:
                writer = csv.writer(stream, delimiter="\t")
                writer.writerow(checker.FIELDS)
                writer.writerow(["copy", "scripts/ios/profile.py", "https://github.com/a/b", digest,
                                 "scripts/ios/profile.py", "yes", digest, "2026-11-10"])
                writer.writerow(["copy", "copy.txt", "https://github.com/a/b", digest,
                                 "copy.txt", "no", digest, "2026-11-10"])
            subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            errors, _ = checker.validate(root)
            self.assertIn("PROVENANCE.tsv must contain at least one model row", errors)

    def test_full_diff_rename_and_post_move_change_are_followed(self):
        spec = importlib.util.spec_from_file_location(
            "triage_upstream", ROOT / "scripts/triage-upstream.py"
        )
        triage = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(triage)
        fixture = ROOT / "scripts/tests/fixtures/provenance-rename.tsv"
        rows = [line.split("\t") for line in fixture.read_text().splitlines()
                if line and not line.startswith("#")]

        old_path = "crates/sheprd-gpui/src/core_terminals/panes.rs"
        current, touched, deleted = triage.advance_path(
            old_path, [(rows[0][0], rows[0][1], rows[0][2])]
        )
        self.assertEqual(current, "crates/sheprd-session/src/panes.rs")
        self.assertTrue(touched)
        self.assertFalse(deleted)

        # The path-filtered negative fixture sees only D and cannot find the
        # destination. The full diff above preserves the R100 pair.
        current, touched, deleted = triage.advance_path(
            old_path, [(rows[1][0], rows[1][1], None)]
        )
        self.assertEqual(current, old_path)
        self.assertTrue(touched)
        self.assertTrue(deleted)

        current, touched, deleted = triage.advance_path(
            "crates/sheprd-session/src/panes.rs", [(rows[2][0], rows[2][1], None)]
        )
        self.assertEqual(current, "crates/sheprd-session/src/panes.rs")
        self.assertTrue(touched)
        self.assertFalse(deleted)


if __name__ == "__main__":
    unittest.main()
