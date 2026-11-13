"""Record the real bundle entry's temp template without building or signing."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class BundleTemp(unittest.TestCase):
    def check_template(self, mode):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            tools = root / 'tools'
            tools.mkdir()
            calls = root / 'native-calls'
            calls.write_text('')
            template_log = root / 'mktemp.json'
            tmp = root / 'run tmp'
            override = root / 'explicit tmp'
            tmp.mkdir()
            override.mkdir()
            expected_root = override if mode == 'override' else tmp if mode == 'tmpdir' else Path('/tmp')
            expected = str(expected_root / 'herdr-ios-bundle.XXXXXX')
            # plutil is a read-only input adapter; every build/native tool rejects.
            scripts = {
                'plutil': 'assert sys.argv[1:5] == ["-extract", "MinimumOSVersion", "raw", "-o"]\nprint("18.0")\n',
                'mktemp': 'Path(os.environ["TEST_TEMPLATE_LOG"]).write_text(json.dumps(sys.argv[1:]))\n'
                          'if sys.argv[1:] != ["-d", os.environ["TEST_TEMPLATE"]]:\n'
                          '    sys.exit("TEST_OUTSIDE_TEMPLATE_REJECTED")\n'
                          'print("TEST_TEMPLATE_RECORDED", file=sys.stderr)\nsys.exit(91)\n',
            }
            for name in ('cargo', 'rustc', 'rustup', 'xcrun', 'xcodebuild', 'xcodegen', 'codesign', 'security'):
                scripts[name] = ('with open(os.environ["TEST_NATIVE_CALLS"], "a") as out:\n'
                                 '    out.write(json.dumps(sys.argv) + "\\n")\n'
                                 'sys.exit("TEST_NATIVE_REJECTED")\n')
            for name, body in scripts.items():
                path = tools / name
                path.write_text(f'#!{sys.executable}\nimport json, os, sys\nfrom pathlib import Path\n' + body)
                path.chmod(0o755)
            env = {k: v for k, v in os.environ.items()
                   if not k.startswith(('YEBISU_', 'HERDR_')) and k != 'TMPDIR'}
            env.update(PATH=str(tools) + os.pathsep + os.environ['PATH'],
                       TEST_TEMPLATE_LOG=str(template_log), TEST_TEMPLATE=expected,
                       TEST_NATIVE_CALLS=str(calls))
            if mode in ('tmpdir', 'override'):
                env['TMPDIR'] = str(tmp)
            if mode == 'override':
                env['YEBISU_BUNDLE_TEMP_ROOT'] = str(override)
            result = subprocess.run([shutil.which('bash'), str(Path(__file__).with_name('bundle.sh')),
                                     '--package', 'yebisu', '--bin', 'yebisu'],
                                    env=env, text=True, capture_output=True, timeout=15)
            recorded = json.loads(template_log.read_text())
            if destination := os.environ.get('YEBISU_HELPER_WITNESS_DIR'):
                (Path(destination) / ('bundle-temp-' + mode + '.json')).write_text(json.dumps(dict(
                    expected=expected, argv=recorded, returncode=result.returncode,
                    stderr=result.stderr, native_calls=calls.read_text()), indent=2))
            self.assertEqual(recorded, ['-d', expected])
            self.assertEqual(result.returncode, 91)
            self.assertIn('TEST_TEMPLATE_RECORDED', result.stderr)
            self.assertEqual(calls.read_text(), '')
            self.assertEqual(list(tmp.iterdir()), [])
            self.assertEqual(list(override.iterdir()), [])

    def test_p11_tmpdir_only(self):
        self.check_template('tmpdir')

    def test_explicit_override_precedes_tmpdir(self):
        self.check_template('override')

    def test_unset_variables_record_tmp_fallback_without_creating_it(self):
        self.check_template('unset')


if __name__ == '__main__':
    unittest.main()
