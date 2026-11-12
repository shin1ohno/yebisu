"""Pure path/artifact checks; never invoke Cargo, signing, Xcode or Simulator."""
import copy
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import tempfile
import json
import os
import shutil

spec = importlib.util.spec_from_file_location("provenance", Path(__file__).with_name("bundle-provenance.py"))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


class ArtifactBinding(unittest.TestCase):
    def fixture(self):
        source = Path('/owned/source')
        target = Path('/owned/target')
        item = dict(reason="compiler-artifact", executable=str(target / 'aarch64-apple-ios-sim/debug/yebisu'),
                    package_id="path+file:///owned/source/crates/yebisu#0.1.0",
                    manifest_path=str(source / 'crates/yebisu/Cargo.toml'),
                    target=dict(name="yebisu", kind=["bin"], src_path=str(source / 'crates/yebisu/src/main.rs')),
                    profile=dict(test=False))
        return item, source, target

    def test_primary_actual_cargo_message_shape(self):
        item, source, target = self.fixture()
        selected, executable = p.select_artifact([dict(reason="build-finished", success=True), item], source, target)
        self.assertEqual(selected, item)
        self.assertEqual(str(executable), item['executable'])

    def test_foreign_checkout_manifest_and_source(self):
        for field in ['manifest_path', 'src_path']:
            item, source, target = self.fixture()
            if field == 'manifest_path':
                item[field] = '/foreign/Cargo.toml'
            else:
                item['target'][field] = '/foreign/main.rs'
            with self.assertRaisesRegex(ValueError, 'foreign'):
                p.select_artifact([item], source, target)

    def test_wrong_target_duplicate_test_and_device_artifacts(self):
        for change, reason in [('foreign', 'outside owned'), ('duplicate', 'one actual'),
                               ('test', 'test artifact'), ('device', 'not Simulator')]:
            item, source, target = self.fixture()
            messages = [item]
            if change == 'foreign': item['executable'] = '/foreign/yebisu'
            if change == 'duplicate': messages.append(copy.deepcopy(item))
            if change == 'test': item['profile']['test'] = True
            if change == 'device': item['executable'] = str(target / 'aarch64-apple-ios/debug/yebisu')
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, reason):
                p.select_artifact(messages, source, target)

    def test_installed_hash_and_uuid_must_both_match(self):
        original = dict(sha256='a', uuid=dict(values=[['UUID', 'arm64']]))
        p.same_binary(original, copy.deepcopy(original))
        for other in [dict(sha256='b', uuid=original['uuid']),
                      dict(sha256='a', uuid=dict(values=[['FOREIGN', 'arm64']]))]:
            with self.assertRaisesRegex(ValueError, 'hash/UUID'):
                p.same_binary(original, other)


class PhaseAdapter(unittest.TestCase):
    def test_actual_json_file_copy_sign_install_binding_and_foreign_install(self):
        # Executable bytes/UUID are controlled fixtures, not a Mach-O/runtime claim.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source, target, evidence = [root / name for name in ['source', 'target', 'evidence']]
            evidence.mkdir()
            main = source / 'crates/yebisu/src/main.rs'
            main.parent.mkdir(parents=True)
            main.write_text('fixture source')
            executable = target / 'aarch64-apple-ios-sim/debug/yebisu'
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b'compiled')
            executable.with_suffix('.d').write_text(str(main) + ' ' + str(main.parent / 'p15_settings_view.rs'))
            item = dict(reason='compiler-artifact', executable=str(executable),
                        manifest_path=str(source / 'crates/yebisu/Cargo.toml'),
                        package_id='path+' + (source / 'crates/yebisu').as_uri() + '#0.1.0',
                        target=dict(name='yebisu', kind=['bin'], src_path=str(main)), profile=dict(test=False))
            (evidence / 'cargo.jsonl').write_text(json.dumps(item) + '\n')
            def cmd(*args):
                return 'f' * 40 if args[-2:] == ('rev-parse', 'HEAD') else ''
            def phase(name, *args):
                with patch('sys.argv', ['provenance', name, '--directory', str(evidence), *args]):
                    p.main()
            with patch.dict(os.environ, YEBISU_OWN_ROOT=str(root), CARGO_TARGET_DIR=str(target), YEBISU_EXPECTED_HEAD='f'*40), \
                 patch.object(p, 'command', side_effect=cmd), \
                 patch.object(p.subprocess, 'check_output', return_value=b'crates/yebisu/src/main.rs\0'), \
                 patch.object(p, 'uuids', return_value=dict(raw='fixture UUID', values=[['UUID', 'arm64']])):
                phase('init', '--source', str(source))
                phase('artifact', '--executable', str(executable))
                app = executable.parent / 'yebisu.app'
                app.mkdir()
                shutil.copyfile(executable, app / 'yebisu')
                phase('copy', '--app', str(app))
                (app / 'yebisu').write_bytes(b'signed fixture')
                phase('signed', '--app', str(app))
                installed = root / 'installed.app'
                shutil.copytree(app, installed)
                phase('installed', '--app', str(installed), '--device', 'leased-fixture-uuid')
                data = json.loads((evidence / 'binding.json').read_text())
                self.assertEqual(data['phases'], ['init', 'artifact', 'copy', 'signed', 'installed'])
                self.assertEqual(data['compiler_artifact'], item)
                self.assertEqual(data['installed']['sha256'], p.digest(installed / 'yebisu'))
                (installed / 'yebisu').write_bytes(b'foreign build')
                with self.assertRaisesRegex(ValueError, 'hash/UUID'):
                    phase('installed', '--app', str(installed), '--device', 'leased-fixture-uuid')


if __name__ == '__main__':
    unittest.main()
