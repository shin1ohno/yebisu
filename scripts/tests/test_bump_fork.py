"""Validation for the intentionally narrow fork-bump recipe."""

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("bump_fork", ROOT / "scripts/bump-fork.py")
bump = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bump)


class BumpForkTests(unittest.TestCase):
    def test_rejects_short_or_non_hex_revision(self):
        self.assertEqual(bump.main(["bump-fork.py", "abc"]), 2)

    def test_lock_package_scan_only_selects_the_fork(self):
        lock = '''[[package]]\nname = "sheprd-client"\nversion = "0.1.0"\nsource = "git+https://github.com/shin1ohno/herdr-gpui?rev=deadbeef#deadbeef"\n\n[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n'''
        self.assertEqual(list(bump.package_blocks(lock)), ["sheprd-client"])


if __name__ == "__main__":
    unittest.main()
