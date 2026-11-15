"""SDK checkout verification of compiler_inputs.py against throwaway git repos.

Fixtures are local repositories made by the tests. No Cargo, no compiler and no
network. The symlink cases mirror herdr-gpui 398c132, which tracks
`CLAUDE.md -> AGENTS.md` (mode 120000) and made the first oracle reject a valid
checkout.
"""
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location('compiler_inputs', HERE / 'compiler_inputs.py')
c = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(c)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def snapshot(root, sdk, revision):
    """Run the oracle's real entry point, metadata_snapshot, over `sdk` frozen at `revision`."""
    source = Path(root) / 'source'
    (source / 'crates/yebisu').mkdir(parents=True, exist_ok=True)
    origin = 'git+https://example.invalid/sdk?rev=' + revision + '#' + revision
    (source / 'Cargo.toml').write_text('[workspace]\nmembers=["crates/yebisu"]\n')
    (source / 'crates/yebisu/Cargo.toml').write_text('[package]\nname="yebisu"\nversion="0.1.0"\n')
    (source / 'Cargo.lock').write_text('[[package]]\nname="sdk"\nversion="1.0.0"\nsource="' + origin + '"\n')
    app_id = 'path+' + (source / 'crates/yebisu').as_uri() + '#0.1.0'
    sdk_id = origin.replace('#' + revision, '#sdk@1.0.0')
    metadata = dict(workspace_root=str(source), packages=[
        dict(id=app_id, name='yebisu', version='0.1.0', source=None,
             manifest_path=str(source / 'crates/yebisu/Cargo.toml'), targets=[]),
        dict(id=sdk_id, name='sdk', version='1.0.0', source=origin,
             manifest_path=str(Path(sdk) / 'Cargo.toml'), targets=[])],
        resolve=dict(nodes=[
            dict(id=app_id, features=[], deps=[dict(name='sdk', pkg=sdk_id)]),
            dict(id=sdk_id, features=[], deps=[])]))
    expected = dict(sdk=[dict(name='sdk', url='https://example.invalid/sdk', revision=revision)])
    packages = c.metadata_snapshot(metadata, source, expected)
    return packages, packages[sdk_id]


class Checkout(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.top = self.root / 'sdk'
        self.top.mkdir()
        (self.top / 'Cargo.toml').write_text('[package]\nname="sdk"\nversion="1.0.0"\n')
        (self.top / 'AGENTS.md').write_text('agents\n')
        (self.top / 'lib.rs').write_text('pub fn api() {}\n')
        (self.top / 'src').mkdir()
        (self.top / 'src/a.rs').write_text('pub fn a() {}\n')
        (self.top / 'tool.sh').write_text('#!/bin/sh\n')
        (self.top / 'tool.sh').chmod(0o755)

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.top), '-c', 'user.name=Fixture',
                               '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', *args],
                              check=True, capture_output=True, text=True).stdout.strip()

    def freeze(self):
        self.git('init', '-q')
        self.git('add', '-A')
        self.git('commit', '-qm', 'fixture: freeze sdk')
        self.revision = self.git('rev-parse', 'HEAD')
        return self.revision

    def verify(self):
        entry = snapshot(self.root, self.top, self.revision)[1]
        return entry['blobs'], entry.get('links', {})

    def freeze_with_claude_link(self):
        # herdr-gpui 398c132 tracks CLAUDE.md -> AGENTS.md (mode 120000)
        os.symlink('AGENTS.md', self.top / 'CLAUDE.md')
        return self.freeze()

    def reject(self, pattern):
        with self.assertRaisesRegex(ValueError, pattern):
            self.verify()

    def replace_with_link(self, name, target):
        path = self.top / name
        path.unlink()
        os.symlink(target, path)

    def commit_link(self, name, target):
        os.symlink(target, self.top / name)
        self.git('add', name)
        self.git('commit', '-qm', 'fixture: link ' + name)
        self.revision = self.git('rev-parse', 'HEAD')

    # C1: a tracked symlink whose link text matches the blob is accepted. The old
    # oracle rejected every tracked symlink before comparing anything.
    def test_c1_tracked_symlink_with_matching_link_text_passes(self):
        self.freeze_with_claude_link()
        blobs, links = self.verify()
        self.assertEqual(links, {str(self.top / 'CLAUDE.md'): digest(b'AGENTS.md')})
        self.assertIn(str(self.top / 'AGENTS.md'), blobs)
        self.assertNotIn(str(self.top / 'CLAUDE.md'), blobs)

    # C2: the link was repointed to another tracked file
    def test_c2_retargeted_symlink_fails(self):
        self.freeze_with_claude_link()
        self.replace_with_link('CLAUDE.md', 'lib.rs')
        self.reject('symlink mismatch: CLAUDE.md')

    # C3: a tracked regular file replaced by a symlink to identical bytes
    def test_c3_regular_file_replaced_by_symlink_fails(self):
        self.freeze()
        (self.top / 'src/b.rs').write_text('pub fn a() {}\n')
        self.git('add', 'src/b.rs')
        self.git('commit', '-qm', 'fixture: copy')
        self.revision = self.git('rev-parse', 'HEAD')
        self.replace_with_link('src/b.rs', 'a.rs')
        self.reject('blob mismatch: src/b.rs')

    # C4: a tracked symlink replaced by a regular file holding the link text
    def test_c4_symlink_replaced_by_regular_file_fails(self):
        self.freeze_with_claude_link()
        (self.top / 'CLAUDE.md').unlink()
        (self.top / 'CLAUDE.md').write_bytes(b'AGENTS.md')
        self.reject('symlink mismatch: CLAUDE.md')

    # C5: a gitlink (mode 160000) is not a file the compiler may read
    def test_c5_gitlink_fails(self):
        self.freeze()
        self.git('update-index', '--add', '--cacheinfo', '160000,' + self.revision + ',vendor')
        self.git('commit', '-qm', 'fixture: gitlink')
        self.revision = self.git('rev-parse', 'HEAD')
        self.reject('entry mode unsupported: vendor')

    # C6: the index mode differs from the frozen revision (regular file made executable)
    def test_c6_index_mode_differs_from_revision_fails(self):
        self.freeze()
        self.git('update-index', '--chmod=+x', 'lib.rs')
        self.reject('index differs from revision: lib.rs')

    def test_c6_index_blob_differs_from_revision_fails(self):
        self.freeze()
        (self.top / 'lib.rs').write_text('pub fn api() { evil() }\n')
        self.git('add', 'lib.rs')
        self.reject('index differs from revision: lib.rs')

    # C7: untracked and ignored files are refused, Cargo's own marker is not
    def test_c7_untracked_file_fails(self):
        self.freeze()
        (self.top / 'extra.rs').write_text('fn extra() {}\n')
        self.reject('untracked file: extra.rs')

    def test_c7_ignored_file_fails(self):
        (self.top / '.gitignore').write_text('generated.rs\n')
        self.freeze()
        (self.top / 'generated.rs').write_text('fn generated() {}\n')
        self.reject('untracked file: generated.rs')

    def test_c7_cargo_marker_is_allowed(self):
        self.freeze()
        (self.top / '.cargo-ok').write_text('{"v":1}')
        self.verify()

    # C8: a parent directory of a tracked file is a symlink to a copy with the same bytes
    def test_c8_intermediate_directory_symlink_fails(self):
        self.freeze()
        copy = self.root / 'elsewhere'
        copy.mkdir()
        (copy / 'a.rs').write_text('pub fn a() {}\n')
        (self.top / 'src/a.rs').unlink()
        (self.top / 'src').rmdir()
        os.symlink(copy, self.top / 'src')
        self.reject('crosses a symlink or non-directory: src/a.rs')

    # C9: link text equals the frozen blob, but the destination is outside the checkout
    def test_c9_target_outside_checkout_fails(self):
        self.freeze()
        self.commit_link('escape.md', '../outside.md')
        self.reject('leaves the checkout: escape.md')

    def test_c9_absolute_target_fails(self):
        self.freeze()
        self.commit_link('absolute.md', '/etc/hostname')
        self.reject('target unsupported: absolute.md')

    def test_c9_dotdot_after_a_component_fails(self):
        self.freeze()
        self.commit_link('indirect.md', 'src/../AGENTS.md')
        self.reject('target unsupported: indirect.md')

    # C10: link text equals the frozen blob, but the destination is not a tracked file
    def test_c10_untracked_target_fails(self):
        self.freeze()
        (self.top / 'local.md').write_text('local\n')
        self.commit_link('dangling.md', 'local.md')
        self.reject('untracked file: local.md')

    def test_c10_missing_target_fails(self):
        self.freeze()
        self.commit_link('dangling.md', 'nowhere.md')
        self.reject('target is not tracked: dangling.md')

    def test_c10_directory_target_fails(self):
        self.freeze()
        self.commit_link('dir.md', 'src')
        self.reject('target is not tracked: dir.md')

    def test_chain_to_regular_file_passes_and_cycle_fails(self):
        self.freeze_with_claude_link()
        self.commit_link('second.md', 'CLAUDE.md')
        blobs, links = self.verify()
        self.assertEqual(len(links), 2)
        os.symlink('cycle-b.md', self.top / 'cycle-a.md')
        os.symlink('cycle-a.md', self.top / 'cycle-b.md')
        self.git('add', '-A')
        self.git('commit', '-qm', 'fixture: cycle')
        self.revision = self.git('rev-parse', 'HEAD')
        self.reject('chain too long or cyclic')


class Snapshot(unittest.TestCase):
    """metadata_snapshot and unchanged() carry the symlink record through."""

    def test_snapshot_records_links_and_unchanged_detects_a_repoint(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve()
            sdk = root / 'sdk'
            sdk.mkdir()
            (sdk / 'Cargo.toml').write_text('[package]\nname="sdk"\nversion="1.0.0"\n')
            (sdk / 'AGENTS.md').write_text('agents\n')
            os.symlink('AGENTS.md', sdk / 'CLAUDE.md')
            revision = self.commit(sdk)
            packages, entry = snapshot(root, sdk, revision)
            self.assertEqual(entry['links'], {str(sdk / 'CLAUDE.md'): digest(b'AGENTS.md')})
            c.unchanged(packages, [])
            (sdk / 'CLAUDE.md').unlink()
            os.symlink('Cargo.toml', sdk / 'CLAUDE.md')
            with self.assertRaisesRegex(ValueError, 'SDK symlink changed during build/install'):
                c.unchanged(packages, [])

    @staticmethod
    def commit(root):
        for args in (['init', '-q'], ['add', '.'], ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                                                   '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture: freeze']):
            subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True)
        return subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()


if __name__ == '__main__':
    unittest.main()
