"""Fork updates exercise manifest parsing, Cargo boundary and rollback."""

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("bump_fork", ROOT / "scripts/bump-fork.py")
bump = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bump)
OLD = "1" * 40
NEW = "2" * 40
MANIFEST = '''[workspace.dependencies]
sheprd-client = { git = "https://github.com/shin1ohno/herdr-gpui", rev = "REV", features = ["ssh"] }
serde = { version = "1" }
[workspace.dependencies.sheprd-terminal]
rev = "REV" # valid table-form rev before git
features = ["default"]
git = "https://github.com/shin1ohno/herdr-gpui"
[target.'cfg(target_os = "ios")'.dependencies.herdr-ssh]
git = 'https://github.com/shin1ohno/herdr-gpui'
rev = 'REV'
'''.replace("REV", OLD)


def lock(revision):
    return '''[[package]]
name = "sheprd-client"
version = "0.1.0"
source = "git+https://github.com/shin1ohno/herdr-gpui?rev=REV#REV"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
'''.replace("REV", revision)


class BumpForkTests(unittest.TestCase):
    def test_rejects_short_or_non_hex_revision(self):
        self.assertEqual(bump.main(["bump-fork.py", "abc"]), 2)

    def test_lock_package_scan_only_selects_the_fork(self):
        self.assertEqual(list(bump.package_blocks(lock(OLD))), ["sheprd-client"])

    def run_update(self, failure=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "Cargo.toml"
            manifest.write_text(MANIFEST)
            nested = root / "crates/yebisu/Cargo.toml"
            nested.parent.mkdir(parents=True)
            nested.write_text('[dependencies]\nsession = { git = "' + bump.FORK_URL + '", rev = "' + OLD + '" }\n')
            lockfile = root / "Cargo.lock"
            lockfile.write_text(lock(OLD))
            originals = {p: p.read_bytes() for p in (manifest, nested, lockfile)}
            calls = []

            def cargo(*args):
                calls.append(args)
                self.assertEqual([dep["rev"] for dep in bump.fork_dependencies(manifest.read_text())], [NEW] * 3)
                if args[1] == "update":
                    lockfile.write_text(lock(NEW))
                if args[1] == "metadata":
                    if failure == "cargo":
                        raise subprocess.CalledProcessError(1, args)
                    if failure == "lock":
                        lockfile.write_text(lock(OLD))
                    if failure == "manifest":
                        nested.write_text(nested.read_text().replace(NEW, OLD))

            with patch.object(bump, "ROOT", root), patch.object(bump, "guarded", cargo):
                if failure:
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        bump.main(["bump-fork.py", NEW])
                    self.assertEqual({p: p.read_bytes() for p in originals}, originals)
                    print("D3 negative: {} failure rolled back all manifests and Cargo.lock byte-for-byte".format(failure))
                else:
                    self.assertEqual(bump.main(["bump-fork.py", NEW]), 0)
                    self.assertEqual([dep["rev"] for dep in bump.fork_dependencies(nested.read_text())], [NEW])
                    self.assertEqual(lockfile.read_text(), lock(NEW))
                    # An already-current revision still validates locked state.
                    self.assertEqual(bump.main(["bump-fork.py", NEW]), 0)
                    print("D3 normal: inline/table/target and multi-manifest dependencies all at one rev")
            self.assertIn(("cargo", "update", "-p", "sheprd-client", "--precise", NEW), calls)
            self.assertIn(("cargo", "metadata", "--locked", "--format-version", "1"), calls)

    def test_update_inline_and_table_dependencies(self):
        self.run_update()

    def test_cargo_failure_rolls_back(self):
        self.run_update("cargo")

    def test_mixed_lock_revisions_roll_back(self):
        self.run_update("lock")

    def test_manifest_revision_mismatch_rolls_back(self):
        self.run_update("manifest")

    def test_metadata_that_resembles_dependencies_is_preserved(self):
        text = MANIFEST + '\n[package.metadata.dependencies]\na = { git = "' + bump.FORK_URL + '", rev = "' + OLD + '" }\n'
        updated, count = bump.update_manifest(text, NEW)
        self.assertEqual(count, 3)
        self.assertIn('rev = "' + OLD + '"', updated)

    def test_unpinned_fork_is_rejected_before_writes(self):
        with self.assertRaisesRegex(ValueError, "pinned rev"):
            bump.update_manifest('[dependencies]\na = { git = "' + bump.FORK_URL + '" }\n', NEW)
        print("D3 negative: unpinned fork rejected before writes")


if __name__ == "__main__":
    unittest.main()
