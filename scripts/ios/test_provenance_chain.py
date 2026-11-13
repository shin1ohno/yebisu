"""Real session/bundle adapters with fake tools. Never invoke native tools/Cargo.

Fixtures exercise arguments, filesystem effects and receiving copies. They are
not evidence of a real compiler, codesign, Simulator, SDK or native application.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import uuid
from unittest.mock import patch

assert sys.dont_write_bytecode, 'run this suite with python3 -B'
HERE = Path(__file__).parent


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value


p = module('provenance', 'bundle-provenance.py')
f = module('fixture', 'test_macho_identity.py')
s = p.session
c = p.compiler

# Primary historical raw Cargo shape; normalized paths only in the test below.
# fd43cf1a8dfd4289/provenance-5/build.stdout:1 SHA256(line)=fa61b8c4ca69876f348abef624217fcc775b32c994df772ae922a5a4fe3f607a
HISTORICAL_BUILD = json.loads('{"reason":"compiler-artifact","package_id":"registry+https://github.com/rust-lang/crates.io-index#proc-macro2@1.0.107","manifest_path":"/Users/shin1ohno/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/proc-macro2-1.0.107/Cargo.toml","target":{"kind":["custom-build"],"crate_types":["bin"],"name":"build-script-build","src_path":"/Users/shin1ohno/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/proc-macro2-1.0.107/build.rs","edition":"2021","doc":false,"doctest":false,"test":false},"profile":{"opt_level":"0","debuginfo":0,"debug_assertions":true,"overflow_checks":true,"test":false},"features":["default","proc-macro"],"filenames":["/Users/shin1ohno/ManagedProjects/cos-fd43cf1a8dfd4289-2/.git/cargo-target-8d3-provenance-5/debug/build/proc-macro2-88f23fd5c1bca07a/build-script-build"],"executable":null,"fresh":true}')

# Each name is a fixture Python program, not a native command or compiler.
FAKE_TOOL = r'''#!/usr/bin/env python3
import fcntl, json, os, pathlib, plistlib, shutil, subprocess, sys
P=pathlib.Path; tool=P(sys.argv[0]).name; a=sys.argv[1:]; root=P(os.environ['FIXTURE_ROOT'])
source=root/'source'; target=P(os.environ['CARGO_TARGET_DIR']); directory=root/'run'
meta=json.loads((root/'metadata.json').read_text())
def option(flag):
 for i,x in enumerate(a):
  if x==flag: return a[i+1]
  if x.startswith(flag+'='): return x[len(flag)+1:]
def log(action):
 locked=False
 if (directory/'owner.lock').exists():
  with (directory/'owner.lock').open() as lock:
   try: fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
   except BlockingIOError: locked=True
 with (root/'calls.jsonl').open('a') as out: out.write(json.dumps(dict(tool=tool,action=action,locked=locked,argv=a))+'\n')
log('enter')
if tool=='rustup': print('aarch64-apple-ios-sim')
elif tool=='plutil':
 if a[0]=='-extract': print('17.0')
 elif a[0]=='-replace':
  path=P(a[-1]); data=plistlib.loads(path.read_bytes()); data[a[1]]=json.loads(a[3]) if a[2]=='-json' else a[3]; path.write_bytes(plistlib.dumps(data))
 elif a[0]!='-lint': raise RuntimeError(a)
elif tool=='fake-rustc':
 name=option('--crate-name'); out=P(option('--out-dir'))
 output=out/({'sdk':'libsdk-unit.rlib','build_script_build':'build_script_build-unit','pm':'libpm-unit.dylib'}.get(name,'yebisu-unit'))
 if '--print=file-names' in a: print(output.name)
 else:
  dep=out/(name+'-unit.d'); src=next(x for x in a if x.endswith('.rs'))
  output.write_bytes((root/'unsigned').read_bytes() if name=='yebisu' else ('compiler-'+name).encode())
  inputs=[src]
  if os.environ.get('FIXTURE_GENERATED'):
   generated=P(os.environ['OUT_DIR'])/'generated.rs'; inputs.append(str(generated)); generated.write_text('changed during compiler')
  lhs=str(output) if not os.environ.get('FIXTURE_WRONG_LHS') else str(out/'foreign-object')
  dep.write_text(str(dep)+': '+' '.join(inputs)+'\n\n'+lhs+': '+' '.join(inputs)+'\n')
elif tool=='cargo':
 if a[0]=='metadata': print(json.dumps(meta))
 else:
  if 'RUSTC_WRAPPER' not in os.environ:
   final=P(os.environ['FIXTURE_GENERIC_OUTPUT']); print(json.dumps(dict(reason='compiler-artifact',package_id='path+file://fixture#0.1.0',target=dict(name=option('--bin') or option('--example'),kind=['bin'] if '--bin' in a else ['example']),executable=str(final))))
  else:
   items=[]
   for package in meta['packages'][::-1]:
    t=package['targets'][0]; name=t['name'].replace('-','_'); host=t['kind'] in (['custom-build'],['proc-macro']); out=target/('debug/deps' if host else 'aarch64-apple-ios-sim/debug/deps'); out.mkdir(parents=True,exist_ok=True)
    args=[str(root/'tools/fake-rustc'),'--crate-name',name,t['src_path'],'--crate-type',t['crate_types'][0],'--emit=dep-info,link','--out-dir',str(out)]
    if not host: args+=['--target','aarch64-apple-ios-sim']
    if name=='yebisu': args+=['--extern','sdk='+str(out/'libsdk-unit.rlib')]
    env=dict(os.environ,CARGO_MANIFEST_DIR=str(P(package['manifest_path']).parent))
    if os.environ.get('FIXTURE_GENERATED'):
     generated=target/'generated'; generated.mkdir(exist_ok=True); (generated/'generated.rs').write_text('before compiler'); env['OUT_DIR']=str(generated)
    result=subprocess.run([os.environ['RUSTC_WRAPPER'],*args],env=env)
    if result.returncode: sys.exit(result.returncode)
    output=out/({'sdk':'libsdk-unit.rlib','build_script_build':'build_script_build-unit','pm':'libpm-unit.dylib'}.get(name,'yebisu-unit'))
    executable=None
    if name=='build_script_build':
     final=target/'debug/build/proc-macro2-unit/build-script-build'; final.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(output,final); output=final
    if name=='yebisu':
     final=out.parent/('foreign-yebisu' if os.environ.get('FIXTURE_WRONG_OUTPUT') else 'yebisu'); shutil.copyfile(output,final); output=final; executable=str(final)
    profile=dict(opt_level='0',debuginfo=0,debug_assertions=True,overflow_checks=True,test=False)
    if os.environ.get('FIXTURE_WRONG_PROFILE') and name=='yebisu': profile['opt_level']='3'
    items.append(dict(reason='compiler-artifact',package_id=package['id'],manifest_path=package['manifest_path'],target=t,profile=profile,features=[],filenames=[str(output)],executable=executable,fresh=False))
   for i in items: print(json.dumps(i))
   for package in meta['packages']:
    if package['targets'][0]['kind']==['custom-build']:
     out=target/'debug/build/proc-macro2-run/out'; out.mkdir(parents=True,exist_ok=True)
     print(json.dumps(dict(reason='build-script-executed',package_id=package['id'],linked_libs=[],linked_paths=[],cfgs=[],env=[],out_dir=str(out))))
   print(json.dumps(dict(reason='build-finished',success=True)))
elif tool=='codesign':
 if '--force' in a: (P(a[-1])/'yebisu').write_bytes((root/'signed').read_bytes())
elif tool=='xcrun':
 if a[0]=='dwarfdump': print('UUID: 00112233-4455-6677-8899-AABBCCDDEEFF (arm64) '+a[-1])
 elif a[0:2]==['simctl','install']:
  if json.loads((directory/'binding.json').read_text())['phases'][-1]!='preinstall': raise RuntimeError('preinstall missing')
  shutil.copytree(a[-1],root/'installed.app')
 elif a[0:2]==['simctl','get_app_container']: print(root/'installed.app')
 else: raise RuntimeError(a)
else: raise RuntimeError(tool)
'''


class Chain(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.source, self.target, self.directory, self.sdk = [self.root / n for n in ('source', 'target', 'run', 'sdk')]
        for path in (self.source, self.target, self.directory, self.sdk, self.root / 'tools', self.root / 'temp'):
            path.mkdir()
        scripts = self.source / 'scripts/ios'; scripts.mkdir(parents=True)
        for name in ('compiler_inputs.py', 'provenance_session.py', 'bundle-provenance.py', 'bundle.sh',
                     'macho_identity.py', 'lib.sh', 'Info.plist'):
            shutil.copy2(HERE / name, scripts / name)
        font = self.source / 'assets/fonts/SauceCodePro'; font.mkdir(parents=True)
        for name in ('Regular', 'Bold', 'Italic', 'BoldItalic'):
            (font / ('SauceCodeProNerdFontMono-' + name + '.ttf')).write_text('font fixture')
        for name in ('LICENSE.txt', 'README.md', 'PROVENANCE.md', 'NERD-FONTS-LICENSE.txt'):
            (font / name).write_text('resource fixture')
        self.main = self.source / 'crates/yebisu/src/main.rs'; self.main.parent.mkdir(parents=True)
        self.main.write_text('fn main() {}'); self.manifest = self.main.parent.parent / 'Cargo.toml'
        self.manifest.write_text('[package]\nname="yebisu"\nversion="0.1.0"\n')
        (self.source / 'Cargo.toml').write_text('[workspace]\nmembers=["crates/yebisu"]\n')
        (self.sdk / 'Cargo.toml').write_text('[package]\nname="sdk"\nversion="1.0.0"\n')
        (self.sdk / 'lib.rs').write_text('pub fn api() {}')
        self.revision = self.commit(self.sdk)
        self.origin = 'git+https://example.invalid/sdk?rev=' + self.revision + '#' + self.revision
        (self.source / 'Cargo.lock').write_text('[[package]]\nname="sdk"\nversion="1.0.0"\nsource="' + self.origin + '"\n')
        self.head = self.commit(self.source)
        def target(name, src, kind):
            return dict(name=name, src_path=str(src), kind=[kind], crate_types=[kind])
        self.item_target = target('yebisu', self.main, 'bin')
        self.app_id = 'path+' + self.manifest.parent.as_uri() + '#0.1.0'
        self.sdk_id = self.origin.replace('#' + self.revision, '#sdk@1.0.0')
        self.metadata = dict(workspace_root=str(self.source), packages=[
            dict(id=self.app_id, name='yebisu', version='0.1.0', source=None,
                 manifest_path=str(self.manifest), targets=[self.item_target]),
            dict(id=self.sdk_id, name='sdk', version='1.0.0', source=self.origin,
                 manifest_path=str(self.sdk / 'Cargo.toml'), targets=[target('sdk', self.sdk / 'lib.rs', 'lib')])],
            resolve=dict(nodes=[dict(id=self.app_id, features=[], deps=[dict(name='sdk', pkg=self.sdk_id, dep_kinds=[dict(kind=None,target=None)])]),
                                dict(id=self.sdk_id, features=[], deps=[])]))
        (self.root / 'metadata.json').write_text(json.dumps(self.metadata))
        (self.root / 'unsigned').write_bytes(f.image())
        (self.root / 'signed').write_bytes(f.image(b'changed signature fixture'))
        for name in ('cargo', 'fake-rustc', 'codesign', 'xcrun', 'plutil', 'rustup'):
            path = self.root / 'tools' / name; path.write_text(FAKE_TOOL); path.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.root / 'tools') + ':' + os.environ['PATH'],
                        FIXTURE_ROOT=str(self.root), CARGO_TARGET_DIR=str(self.target),
                        YEBISU_OWN_ROOT=str(self.root), YEBISU_EXPECTED_HEAD=self.head,
                        YEBISU_BUNDLE_PROVENANCE_DIR=str(self.directory),
                        YEBISU_BUNDLE_TEMP_ROOT=str(self.root / 'temp'), CARGO_HOME=str(self.root / 'cargo-home'),
                        PYTHONDONTWRITEBYTECODE='1')
        for name in ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'HERDR_IOS_TEAM', 'CARGO_BUILD_BUILD_DIR'):
            self.env.pop(name, None)
        self.expected = dict(schema=s.SCHEMA, head=self.head, run_id=str(uuid.uuid4()),
                             package='yebisu', binary='yebisu', target='aarch64-apple-ios-sim',
                             features=[], no_default_features=False, profile_name='dev',
                             profile=dict(opt_level='0', debuginfo=0, debug_assertions=True, overflow_checks=True, test=False),
                             sdk=[dict(name='sdk', url='https://example.invalid/sdk', revision=self.revision)])
        self.device = '11111111-2222-3333-4444-555555555555'
        self.bundle_args = ['--package', 'yebisu', '--bin', 'yebisu']

    def commit(self, root):
        for args in (['init', '-q'], ['add', '.'], ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                                                   '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture: freeze test inputs']):
            subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True)
        return subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()

    def build(self):
        try:
            return s.build_install(self.directory, self.source, self.device, self.bundle_args, self.env, expected=self.expected)
        except subprocess.CalledProcessError as exc:
            # Preserve the exact adapter failure, not an unrelated broad ValueError.
            raise AssertionError(exc.stderr) from exc

    def test_real_session_bundle_wrapper_and_receiving_copy(self):
        result = self.build()
        receipt = json.loads((self.directory / 'binding.json').read_text())
        self.assertEqual(receipt['phases'], ['init', 'metadata', 'artifact', 'copy', 'signed', 'preinstall', 'installed'])
        self.assertEqual(receipt['expected'], self.expected)
        self.assertEqual((self.directory / 'installed.macho').read_bytes(), (self.root / 'signed').read_bytes())
        self.assertEqual(result['installed'], str(self.root / 'installed.app'))
        calls = [json.loads(x) for x in (self.root / 'calls.jsonl').read_text().splitlines()]
        self.assertTrue(all(x['locked'] for x in calls))
        self.assertTrue(any(x['tool'] == 'fake-rustc' for x in calls))
        self.assertTrue(all(x['context'] == 'target' for x in receipt['compiler_inputs']))
        self.assertEqual(receipt['compiler_inputs'][-1]['externs']['sdk']['sha256'], c.sha(self.target / 'aarch64-apple-ios-sim/debug/deps/libsdk-unit.rlib'))
        with (self.directory / 'owner.lock').open() as lock:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)  # Released only after installed receipt.

    def test_historical_custom_build_null_executable_and_host_proc_macro(self):
        raw = copy.deepcopy(HISTORICAL_BUILD)
        self.assertIsNone(raw['executable'])
        registry = self.root / 'registry'; registry.mkdir()
        for name, target_name, kind in (('proc-macro2', 'build-script-build', 'custom-build'), ('fixture-macro', 'pm', 'proc-macro')):
            package = registry / name; package.mkdir()
            manifest = package / 'Cargo.toml'; manifest.write_text('[package]')
            src = package / ('build.rs' if kind == 'custom-build' else 'lib.rs'); src.write_text('fixture')
            origin = 'registry+https://github.com/rust-lang/crates.io-index'
            package_id = origin + '#' + name + '@1.0.107'
            target = dict(raw['target'], name=target_name, kind=[kind], crate_types=['bin' if kind == 'custom-build' else 'proc-macro'], src_path=str(src))
            self.metadata['packages'].append(dict(id=package_id, name=name, version='1.0.107', source=origin,
                                                 manifest_path=str(manifest), targets=[target]))
            # Resolver union can exceed the actual host unit's feature set.
            self.metadata['resolve']['nodes'].append(dict(id=package_id, features=['default', 'proc-macro'], deps=[]))
            with (self.source / 'Cargo.lock').open('a') as out:
                out.write('[[package]]\nname="'+name+'"\nversion="1.0.107"\nsource="'+origin+'"\n')
        self.expected['head'] = self.env['YEBISU_EXPECTED_HEAD'] = self.commit(self.source)
        (self.root / 'metadata.json').write_text(json.dumps(self.metadata))
        self.build()
        receipt = json.loads((self.directory / 'binding.json').read_text())
        hosts = [r for r in receipt['compiler_inputs'] if r['context'] == 'host']
        self.assertEqual({tuple(r['artifact']['target']['kind']) for r in hosts}, {('custom-build',), ('proc-macro',)})
        self.assertTrue(all(r['artifact']['executable'] is None for r in hosts))
        self.assertEqual(len(receipt['build_scripts']), 1)
        self.assertIn('Cargo-trusted', receipt['build_scripts'][0]['rerun_policy'])

    def test_wrong_root_symlink_stale_head_before_first_write(self):
        outside = self.root / 'outside'; outside.mkdir()
        for case in ('wrong-root', 'symlink', 'stale-run', 'head', 'target'):
            env = dict(self.env); directory = self.directory
            if case == 'wrong-root': env['YEBISU_OWN_ROOT'] = str(outside)
            if case == 'symlink':
                directory = self.root / 'alias'; directory.symlink_to(self.directory)
                env['YEBISU_BUNDLE_PROVENANCE_DIR'] = str(directory)
            if case == 'stale-run': (self.directory / 'old').write_text('preserve')
            expected = copy.deepcopy(self.expected)
            if case == 'head': expected['head'] = env['YEBISU_EXPECTED_HEAD'] = 'f' * 40
            if case == 'target': env['CARGO_TARGET_DIR'] = str(self.source)
            before = {str(x): x.read_bytes() for x in self.directory.iterdir() if x.is_file()}
            with self.subTest(case=case), self.assertRaisesRegex(ValueError, 'owned root|canonical|fresh|head|target'):
                s.build_install(directory, self.source, self.device, self.bundle_args, env, expected=expected)
            self.assertEqual(before, {str(x): x.read_bytes() for x in self.directory.iterdir() if x.is_file()})
            self.assertFalse((self.root / 'calls.jsonl').exists())
            self.assertEqual(list(outside.iterdir()), [])
            if case == 'stale-run': (self.directory / 'old').unlink()

    def test_foreign_lhs_fails_at_compiler_output_oracle(self):
        self.env['FIXTURE_WRONG_LHS'] = '1'
        with self.assertRaisesRegex(AssertionError, 'freshly emitted depfile'):
            self.build()
        self.assertFalse((self.root / 'installed.app').exists())

    def test_actual_profile_must_match_expected(self):
        self.env['FIXTURE_WRONG_PROFILE'] = '1'
        with self.assertRaisesRegex(AssertionError, 'expected app profile|actual compiler invocation'):
            self.build()
        self.assertFalse((self.root / 'installed.app').exists())

    def test_selected_cargo_output_must_equal_explicit_target_contract(self):
        self.env['FIXTURE_WRONG_OUTPUT'] = '1'
        with self.assertRaisesRegex(AssertionError, 'wrong selected executable output'):
            self.build()
        self.assertFalse((self.root / 'installed.app').exists())

    def test_generated_input_changed_inside_compiler_is_rejected(self):
        self.env['FIXTURE_GENERATED'] = '1'
        with self.assertRaisesRegex(AssertionError, 'input changed or lacks pre-build snapshot'):
            self.build()
        self.assertFalse((self.root / 'installed.app').exists())

    def test_coherent_lock_metadata_wrong_expected_sdk_rejected(self):
        self.expected['sdk'][0]['revision'] = 'b' * 40
        with self.assertRaisesRegex(AssertionError, 'expected SDK package/revision mismatch'):
            self.build()
        calls = [json.loads(x) for x in (self.root / 'calls.jsonl').read_text().splitlines()]
        self.assertFalse(any(x['tool'] == 'fake-rustc' for x in calls))

    def test_default_p11_guard_preserves_escaped_bundle_before_rm(self):
        escaped = self.root / 'escaped'; escaped.mkdir(); exe = escaped / 'yebisu'; exe.write_text('compiler fixture')
        app = escaped / 'yebisu.app'; app.mkdir(); marker = app / 'preserve'; marker.write_text('do not delete')
        env = dict(self.env, HERDR_IOS_ARTIFACT_ROOT=str(self.target), FIXTURE_GENERIC_OUTPUT=str(exe))
        env.pop('YEBISU_BUNDLE_PROVENANCE_DIR')
        result = subprocess.run(['bash', 'scripts/ios/bundle.sh', *self.bundle_args], cwd=self.source,
                                env=env, text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('cargo artifact escaped', result.stderr)
        self.assertEqual(marker.read_text(), 'do not delete')
        calls = [json.loads(x) for x in (self.root / 'calls.jsonl').read_text().splitlines()]
        self.assertFalse(any(x['tool'] == 'codesign' for x in calls))


if __name__ == '__main__':
    unittest.main()
