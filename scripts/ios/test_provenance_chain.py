"""Host-shaped Cargo/metadata/depfile and full phase tests, no native tools."""
import copy
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value


p = module('provenance', 'bundle-provenance.py')
f = module('fixture', 'test_macho_identity.py')


class Chain(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.source, self.target, self.directory, self.sdk = [self.root / n for n in ('source', 'target', 'evidence', 'sdk')]
        for d in (self.source, self.target, self.directory, self.sdk): d.mkdir()
        self.main = self.source / 'crates/yebisu/src/main.rs'
        self.main.parent.mkdir(parents=True); self.main.write_text('fn main() {}')
        self.manifest = self.main.parent.parent / 'Cargo.toml'; self.manifest.write_text('[package]')
        self.sdk_manifest = self.sdk / 'Cargo.toml'; self.sdk_manifest.write_text('[package]')
        (self.sdk / 'lib.rs').write_text('pub fn api() {}')
        self.origin = 'git+https://example.invalid/sdk?rev=' + 'a'*40 + '#' + 'a'*40
        (self.source / 'Cargo.lock').write_text('[[package]]\nname="sdk"\nversion="1.0.0"\nsource="' + self.origin + '"\n')
        self.executable = self.target / 'aarch64-apple-ios-sim/debug/yebisu'
        self.executable.parent.mkdir(parents=True); self.executable.write_bytes(f.image())
        self.executable.with_suffix('.d').write_text(str(self.executable) + ': ' + str(self.main) + '\n')
        library = self.executable.parent / 'libsdk-abcd.rlib'; library.write_bytes(b'rlib')
        library.with_name('sdk-abcd.d').write_text(str(library) + ': ' + str(self.sdk / 'lib.rs') + '\n')
        self.app_id = 'path+' + self.manifest.parent.as_uri() + '#0.1.0'
        self.sdk_id = self.origin + '#sdk@1.0.0'
        self.item = dict(reason='compiler-artifact', executable=str(self.executable), filenames=[str(self.executable)],
                         manifest_path=str(self.manifest), package_id=self.app_id,
                         target=dict(name='yebisu', kind=['bin'], src_path=str(self.main)),
                         profile=dict(test=False, opt_level='0', debuginfo=2), features=[], fresh=False)
        dep = dict(reason='compiler-artifact', executable=None, filenames=[str(library)],
                   manifest_path=str(self.sdk_manifest), package_id=self.sdk_id,
                   target=dict(name='sdk', kind=['lib'], src_path=str(self.sdk / 'lib.rs')),
                   profile=dict(test=False, opt_level='0'), features=[], fresh=False)
        self.messages = [dep, self.item, dict(reason='build-finished', success=True)]
        self.metadata = dict(workspace_root=str(self.source), packages=[
            dict(id=self.app_id, name='yebisu', version='0.1.0', source=None,
                 manifest_path=str(self.manifest), targets=[self.item['target']]),
            dict(id=self.sdk_id, name='sdk', version='1.0.0', source=self.origin,
                 manifest_path=str(self.sdk_manifest), targets=[dep['target']])],
            resolve=dict(nodes=[dict(id=self.app_id, features=[]), dict(id=self.sdk_id, features=[])]))
        (self.directory / 'cargo.jsonl').write_text('\n'.join(json.dumps(i) for i in self.messages))
        (self.directory / 'metadata.json').write_text(json.dumps(self.metadata))
        self.lock = (self.directory / 'owner.lock').open('x'); self.addCleanup(self.lock.close)
        fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        (self.directory / 'owner.json').write_text(json.dumps(dict(token='fixture', device='leased-device')))
        self.env = patch.dict(os.environ, YEBISU_PROVENANCE_TOKEN='fixture', YEBISU_OWN_ROOT=str(self.root),
                              CARGO_TARGET_DIR=str(self.target), YEBISU_EXPECTED_HEAD='f'*40)
        self.env.start(); self.addCleanup(self.env.stop)
        def command(*args):
            return 'f'*40 if args[-2:] == ('rev-parse', 'HEAD') else ''
        # The git object bytes are frozen before mutation; not derived from the tested file afterwards.
        originals = {'Cargo.toml': b'[package]', 'lib.rs': b'pub fn api() {}'}
        def git(root, *args):
            if args == ('rev-parse', '--show-toplevel'): return str(self.sdk).encode()
            if args == ('rev-parse', 'HEAD'): return b'a'*40
            if args == ('ls-files', '-z'): return b'Cargo.toml\0lib.rs\0'
            if args[0] == 'show': return originals[args[1].split(':', 1)[1]]
            raise AssertionError(args)
        for mocked in (patch.object(p, 'command', side_effect=command),
                       patch.object(p.subprocess, 'check_output', return_value=b'crates/yebisu/src/main.rs\0'),
                       patch.object(p.compiler, 'git', side_effect=git),
                       patch.object(p, 'uuids', return_value=dict(values=[['0123', 'arm64']]))):
            mocked.start(); self.addCleanup(mocked.stop)

    def phase(self, name, *args):
        with patch('sys.argv', ['provenance', name, '--directory', str(self.directory), *args]):
            p.main()

    def copied(self):
        self.phase('init', '--source', str(self.source))
        self.phase('metadata')
        self.phase('artifact', '--executable', str(self.executable))
        self.app = self.directory / 'yebisu.app'; self.app.mkdir()
        shutil.copyfile(self.executable, self.app / 'yebisu')
        self.phase('copy', '--app', str(self.app))

    def test_full_chain_signature_only_and_exact_receiving_copy(self):
        self.copied()
        (self.app / 'yebisu').write_bytes(f.image(b'new signature only' * 7))
        self.phase('signed', '--app', str(self.app))
        self.phase('preinstall', '--app', str(self.app))
        installed = self.root / 'installed.app'; shutil.copytree(self.app, installed)
        self.phase('installed', '--app', str(installed), '--device', 'leased-device')
        receipt = json.loads((self.directory / 'binding.json').read_text())
        self.assertEqual(receipt['phases'], ['init', 'metadata', 'artifact', 'copy', 'signed', 'preinstall', 'installed'])
        self.assertEqual(receipt['compiler_artifact'], self.item)
        self.assertEqual((self.directory / 'installed.macho').read_bytes(), (installed / 'yebisu').read_bytes())
        self.assertIn(str(self.sdk / 'lib.rs'), receipt['compiler_inputs'][0]['inputs'])

    def test_unsigned_tamper_same_uuid_rejected(self):
        self.copied()
        data = bytearray((self.app / 'yebisu').read_bytes()); data[250] ^= 1
        (self.app / 'yebisu').write_bytes(data)
        with self.assertRaisesRegex(ValueError, 'code identity'):
            self.phase('signed', '--app', str(self.app))

    def test_wrong_sdk_pin_rejected_before_artifact(self):
        self.phase('init', '--source', str(self.source))
        metadata = copy.deepcopy(self.metadata)
        metadata['packages'][1]['source'] = self.origin.replace('a'*40, 'b'*40)
        (self.directory / 'metadata.json').write_text(json.dumps(metadata))
        with self.assertRaisesRegex(ValueError, 'Cargo.lock'):
            self.phase('metadata')

    def test_modified_sdk_blob_rejected(self):
        self.phase('init', '--source', str(self.source))
        (self.sdk / 'lib.rs').write_text('foreign SDK')
        with self.assertRaisesRegex(ValueError, 'blob mismatch'):
            self.phase('metadata')

    def test_wrong_src_and_sdk_artifact_and_cache_rejected(self):
        self.phase('init', '--source', str(self.source)); self.phase('metadata')
        for change, message in [('src', 'foreign source'), ('sdk', 'missing from resolved'), ('cache', 'freshly built')]:
            messages = copy.deepcopy(self.messages)
            if change == 'src': messages[1]['target']['src_path'] = '/foreign/main.rs'
            if change == 'sdk': messages[0]['package_id'] = 'foreign-sdk'
            if change == 'cache': messages[1]['fresh'] = True
            (self.directory / 'cargo.jsonl').write_text('\n'.join(json.dumps(i) for i in messages))
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                self.phase('artifact', '--executable', str(self.executable))

    def test_install_checks_resources_and_binary_after_signing(self):
        self.copied(); self.phase('signed', '--app', str(self.app))
        (self.app / 'extra').write_text('unrecorded')
        with self.assertRaisesRegex(ValueError, 'bundle changed'):
            self.phase('preinstall', '--app', str(self.app))

    def test_depfile_false_substring_does_not_count_as_input(self):
        self.phase('init', '--source', str(self.source)); self.phase('metadata')
        foreign = self.main.with_name('main.rs.old'); foreign.write_text('other')
        self.executable.with_suffix('.d').write_text(str(self.executable) + ': ' + str(foreign))
        with self.assertRaisesRegex(ValueError, 'actual target source'):
            self.phase('artifact', '--executable', str(self.executable))

    def test_no_lock_no_owner_no_phase_skip(self):
        with self.assertRaises(FileNotFoundError): self.phase('signed')
        fcntl.flock(self.lock, fcntl.LOCK_UN)
        with self.assertRaisesRegex(ValueError, 'not held'): self.phase('init', '--source', str(self.source))


if __name__ == '__main__':
    unittest.main()
