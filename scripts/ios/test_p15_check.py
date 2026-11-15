import copy
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("p15", Path(__file__).with_name("p15-check.py"))
p15 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p15)


class ThemeOracle(unittest.TestCase):
    def fixture(self):
        def paint(theme="high-contrast", modal=False):
            return dict(kind="terminal", theme=theme, settings_modal=modal)
        records = [dict(kind="settings-loaded", theme="owner-default"),
                   paint("owner-default"), dict(kind="theme-selected", theme="high-contrast"),
                   paint(modal=True), dict(kind="theme-selected", theme="owner-default"),
                   paint("owner-default", modal=True),
                   dict(kind="theme-selected", theme="high-contrast"),
                   paint(modal=True), paint(),
                   dict(kind="settings-lifecycle", phase="Background"),
                   dict(kind="settings-lifecycle", phase="Active"), paint(),
                   dict(kind="settings-loaded", theme="high-contrast"), paint()]
        return records, dict(copy.deepcopy(p15.SEED), theme="high-contrast")

    def test_primary_receiving_shape(self):
        p15.validate(*self.fixture())

    def test_wrong_or_missing_saved_field(self):
        for field in p15.SEED:
            records, disk = self.fixture()
            disk.pop(field)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "exact saved"):
                p15.validate(records, disk)
        records, disk = self.fixture()
        disk["theme"] = "owner-default"
        with self.assertRaisesRegex(ValueError, "exact saved"):
            p15.validate(records, disk)
        disk["theme"] = "high-contrast"
        disk["notifications"]["volume"] = 38
        with self.assertRaisesRegex(ValueError, "exact saved"):
            p15.validate(records, disk)

    def test_missing_real_click_lifecycle_load_and_paints(self):
        for index, reason in [(0, "two real"), (2, "ordered real"),
                              (3, "immediate modal"), (8, "closed-sheet"),
                              (9, "background/return"), (10, "background/return"),
                              (11, "background return"), (12, "two real"),
                              (13, "relaunched painter")]:
            records, disk = self.fixture()
            records.pop(index)
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, reason):
                p15.validate(records, disk)

    def test_stale_theme_paint_is_not_success(self):
        records, disk = self.fixture()
        records[3]["theme"] = "owner-default"
        with self.assertRaisesRegex(ValueError, "immediate modal"):
            p15.validate(records, disk)


class InstalledUIWireOracle(unittest.TestCase):
    def fixture(self):
        ui = [dict(kind="theme-click", label=label) for label in
              ["High contrast", "Owner default", "High contrast"]]
        records = []
        for index, (stage, host, stale) in enumerate([
                ("alpha", "qa-alpha", "qa-beta"), ("beta", "qa-beta", "qa-alpha"),
                ("relaunch-host", "qa-alpha", "qa-beta")]):
            ui.append(dict(kind="host-screen", stage=stage, host=host, stale=stale,
                           time=index + 1.5, lines=["Host: " + host, "File path unavailable",
                           "Unavailable", "File settings only; daemon-effective state unverified"]))
            records.append(dict(kind="terminal", time=index + 1, host=host, settings_modal=True))
        messages = [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [
                    {"TextCommit": "P15-start"}, {"TextCommit": "P15-end"}]}}]
        return records, messages, ui

    def test_installed_receiving_shape(self):
        p15.validate_ui_wire(*self.fixture())

    def test_stale_host_negative(self):
        records, messages, ui = self.fixture()
        ui[4]["lines"][0] = "Host: qa-alpha"
        with self.assertRaisesRegex(ValueError, "stale host"):
            p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        records[1]["host"] = "qa-alpha"
        with self.assertRaisesRegex(ValueError, "current renderer"):
            p15.validate_ui_wire(records, messages, ui)

    def test_wire_leak_and_missing_positive_negative(self):
        for events in [[{"TextCommit": "P15-startLEAKP15-end"}], [],
                       [{"Paste": "P15-startP15-end"}],
                       [{"TextCommit": "P15-endP15-start"}]]:
            records, messages, ui = self.fixture()
            messages[0]["ClientShellPaneInput"]["events"] = events
            with self.subTest(events=events), self.assertRaises(ValueError):
                p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        messages.append({"ClientShellEndpointRequest": {"request": '{"method":"config.reload"}'}})
        with self.assertRaisesRegex(ValueError, "unexpected endpoint"):
            p15.validate_ui_wire(records, messages, ui)


class ProvenanceCaller(unittest.TestCase):
    def test_passes_p15_pin_profile_and_unique_run_to_real_api_shape(self):
        import tomllib
        import uuid
        from unittest.mock import Mock
        session = Mock()
        session.build_install.return_value = {'installed': '/owned/installed.app'}
        env = {'YEBISU_EXPECTED_HEAD': 'a' * 40}
        for _ in range(2):
            result = p15.build_owned(session, Path('/owned/run'), Path('/owned/source'),
                                     '11111111-2222-3333-4444-555555555555', env, 'a' * 40)
            self.assertEqual(result['installed'], '/owned/installed.app')
        first, second = session.build_install.call_args_list
        expected = first.kwargs['expected']
        manifest = tomllib.loads((p15.ROOT / 'Cargo.toml').read_text())
        pins = {p['name']: p['revision'] for p in expected['sdk']}
        for name in ('herdr-uikit', 'herdr-ssh'):
            self.assertEqual(pins[name], manifest['workspace']['dependencies'][name]['rev'])
        self.assertEqual(expected['profile'], dict(opt_level='0', debuginfo=0,
                         debug_assertions=True, overflow_checks=True, test=False))
        self.assertEqual(expected['target'], 'aarch64-apple-ios-sim')
        self.assertEqual(expected['features'], [])
        self.assertEqual(expected['head'], 'a' * 40)
        self.assertEqual(first.args[3], ['--package', 'yebisu', '--bin', 'yebisu'])
        self.assertIs(first.args[4], env)
        uuid.UUID(expected['run_id'])
        self.assertNotEqual(expected['run_id'], second.kwargs['expected']['run_id'])


class MainAdmission(unittest.TestCase):
    """Exercise the real entry point, including writes before the session API."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name).resolve()
        self.own = self.base / 'owned'
        self.source = self.own / 'source'
        scripts = self.source / 'scripts/ios'
        scripts.mkdir(parents=True)
        for name in ('p15-check.py', 'p11-check.py', 'provenance_session.py'):
            shutil.copy2(Path(__file__).with_name(name), scripts / name)
        shutil.copytree(Path(__file__).with_name('smoke-qa'), scripts / 'smoke-qa',
                        ignore=shutil.ignore_patterns('*.xcodeproj', '__pycache__'))
        self.git('init', '-q')
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null',
                 'commit', '-qm', 'fixture: capture exact caller scripts')
        self.head = self.git('rev-parse', 'HEAD').strip()
        self.outside = self.base / 'outside'
        self.outside.mkdir()
        (self.outside / 'sentinel').write_bytes(b'preserve outside bytes\x00')
        (self.outside / 'link').symlink_to('sentinel')
        self.calls = self.base / 'native-calls.jsonl'
        self.calls.write_text('')
        tools = self.base / 'tools'
        tools.mkdir()
        for name in ('bash', 'cargo', 'rustc', 'xcrun', 'xcodebuild', 'xcodegen', 'codesign', 'security'):
            tool = tools / name
            tool.write_text(f'#!{sys.executable}\nimport json, os, sys\n'
                            'with open(os.environ["TEST_NATIVE_CALLS"], "a") as out:\n'
                            '    out.write(json.dumps(sys.argv) + "\\n")\n'
                            'sys.exit("TEST_NATIVE_REJECTED")\n')
            tool.chmod(0o755)
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                        TEST_NATIVE_CALLS=str(self.calls), GIT_OPTIONAL_LOCKS='0')
        self.arguments = ['--own-root', str(self.own), '--expected-head', self.head,
                          '--simulator', '11111111-2222-3333-4444-555555555555']

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.source), *args], text=True)

    def inventory(self, root):
        result = {}

        def visit(path):
            info = path.lstat()
            entry = dict(mode=stat.S_IMODE(info.st_mode), uid=info.st_uid)
            if stat.S_ISLNK(info.st_mode):
                entry.update(type='link', target=os.readlink(path))
            elif stat.S_ISREG(info.st_mode):
                entry.update(type='file', sha256=hashlib.sha256(path.read_bytes()).hexdigest())
            elif stat.S_ISDIR(info.st_mode):
                entry.update(type='directory')
                for child in sorted(path.iterdir()):
                    visit(child)
            else:
                entry.update(type='special')
            result[str(path.relative_to(root))] = entry

        visit(root)
        return result

    def invoke(self, reason=None):
        before = {name: self.inventory(path) for name, path in
                  [('owned', self.own), ('outside', self.outside)]}
        result = subprocess.run([sys.executable, '-I', '-B',
                                 str(self.source / 'scripts/ios/p15-check.py'), *self.arguments],
                                cwd=self.source, env=self.env, text=True, capture_output=True, timeout=15)
        after = {name: self.inventory(path) for name, path in
                 [('owned', self.own), ('outside', self.outside)]}
        calls = self.calls.read_text()
        if destination := os.environ.get('YEBISU_HELPER_WITNESS_DIR'):
            (Path(destination) / (self._testMethodName + '.json')).write_text(json.dumps(dict(
                head=self.head, argv=self.arguments, returncode=result.returncode,
                stdout=result.stdout, stderr=result.stderr, native_calls=calls,
                before=before, after=after), indent=2))
        self.assertNotEqual(result.returncode, 0)
        if reason:
            self.assertIn(reason, result.stderr)
            self.assertEqual(before, after)
            self.assertEqual(calls, '')
        else:
            self.assertIn('TEST_NATIVE_REJECTED', result.stdout + result.stderr)
            recorded = [json.loads(line) for line in calls.splitlines()]
            self.assertEqual(len(recorded), 1)
            self.assertEqual(Path(recorded[0][0]).name, 'bash')
            runs = list((self.own / 'evidence').iterdir())
            self.assertEqual(len(runs), 1)
            for name in ('provenance', 'cargo-target', 'smoke-qa'):
                self.assertTrue((runs[0] / name).is_dir())
            self.assertEqual(before['outside'], after['outside'])

    def test_main_rejects_evidence_symlink_before_writes(self):
        (self.own / 'evidence').symlink_to(self.outside, target_is_directory=True)
        self.invoke('requires canonical absolute path without symlinks')

    def test_main_rejects_dangling_evidence_symlink_before_writes(self):
        (self.own / 'evidence').symlink_to(self.outside / 'absent')
        self.invoke('requires canonical absolute path without symlinks')

    def test_main_rejects_own_alias_before_writes(self):
        alias = self.base / 'alias'
        alias.symlink_to(self.own, target_is_directory=True)
        self.arguments[1] = str(alias)
        self.invoke('requires canonical absolute path without symlinks')

    def test_main_rejects_wrong_head_before_writes(self):
        self.arguments[3] = '0' * 40
        self.invoke('unexpected source checkout/head')

    def test_main_rejects_dirty_source_before_writes(self):
        with (self.source / 'scripts/ios/p11-check.py').open('a') as out:
            out.write('\n# dirty fixture\n')
        self.invoke('tracked source is dirty')

    def test_main_rejects_non_directory_evidence_before_writes(self):
        (self.own / 'evidence').write_text('preserve')
        self.invoke('ownership paths must be directories')

    def test_main_rejects_missing_required_argument_before_writes(self):
        self.arguments = self.arguments[:-2]
        self.invoke('owned source, full expected head and leased simulator UUID required')

    def test_main_rejects_invalid_head_before_writes(self):
        self.arguments[3] = 'g' * 40
        self.invoke('full source head required')

    def test_main_rejects_invalid_simulator_before_writes(self):
        self.arguments[-1] = 'not-a-uuid'
        self.invoke('badly formed hexadecimal UUID string')

    def test_main_canonical_root_reaches_only_rejecting_native_stub(self):
        self.invoke()

    def test_main_existing_evidence_reaches_only_rejecting_native_stub(self):
        (self.own / 'evidence').mkdir()
        self.invoke()


if __name__ == "__main__":
    unittest.main()
