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


# The full adapter fixture is in test_provenance_chain.py.

if __name__ == '__main__':
    unittest.main()
