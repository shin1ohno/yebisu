"""Issue #235: simulator-smoke boots one Simulator before the build and P12 reuses it.

On the xcode-27 image a freshly booted Simulator blocks its first install for
minutes, so the job must not pay that twice: ci-smoke.py boots before the build
and, in GitHub Actions with --p9, hands a passing run's Simulator to p12-ci.py,
which uninstalls the app, runs P12 on it and deletes it.
"""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ci_smoke = load('ci_smoke_handover', 'ci-smoke.py')
p12_ci = load('p12_ci_handover', 'p12-ci.py')

IPAD = {'name': 'iPad Pro 13-inch (M5)', 'isAvailable': True, 'udid': 'smoke-udid',
        'deviceTypeIdentifier': 'com.apple.CoreSimulator.SimDeviceType.iPad-Pro-13-inch-M5-12GB'}
RUNTIME = 'com.apple.CoreSimulator.SimRuntime.iOS-27-2'


def ok(command=None):
    return subprocess.CompletedProcess(command or [], 0, '', '')


def is_delete(command):
    return command[:3] == ['xcrun', 'simctl', 'delete']


class SmokeStepTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.handover = Path(self.directory.name) / 'target' / 'ci-smoke-simulator.txt'
        patcher = patch.object(ci_smoke, 'HANDOVER', self.handover, create=True)
        patcher.start()
        self.addCleanup(patcher.stop)
        # relative_to(ROOT) in the handover line needs the temporary path under ROOT.
        root = patch.object(ci_smoke, 'ROOT', Path(self.directory.name), create=True)
        root.start()
        self.addCleanup(root.stop)

    def smoke(self, *, p9, actions, failing=None):
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs))
            name = Path(command[1]).name if len(command) > 1 else ''
            if failing and name == failing:
                raise subprocess.CalledProcessError(1, command)
            return ok(command)

        environment = {'GITHUB_ACTIONS': 'true'} if actions else {}
        log = io.StringIO()
        with patch.dict(os.environ, environment, clear=False), \
                patch.object(ci_smoke.subprocess, 'check_output', side_effect=[
                    json.dumps({'devices': {RUNTIME: [IPAD]}}), 'smoke-udid\n']), \
                patch.object(ci_smoke.subprocess, 'run', side_effect=run), contextlib.redirect_stdout(log):
            if not actions:
                os.environ.pop('GITHUB_ACTIONS', None)
            if failing:
                with self.assertRaises(subprocess.CalledProcessError):
                    ci_smoke.main(p9=p9)
            else:
                ci_smoke.main(p9=p9)
        return calls, log.getvalue()

    def test_the_simulator_boots_before_smoke_check_builds(self):
        calls, _ = self.smoke(p9=True, actions=True)
        boot, smoke = calls[0], calls[1]
        self.assertEqual(boot[0][:2], ['bash', '-c'])
        self.assertIn('ios_sim_boot "$HERDR_IOS_SIMULATOR"', boot[0][2])
        self.assertEqual(boot[1]['env']['HERDR_IOS_SIMULATOR'], 'smoke-udid')
        self.assertTrue(boot[1]['check'])
        self.assertEqual(Path(smoke[0][1]).name, 'smoke-check.py')

    def test_a_passing_p9_run_in_actions_hands_its_simulator_to_p12(self):
        calls, log = self.smoke(p9=True, actions=True)
        self.assertEqual(self.handover.read_text(), 'smoke-udid\n')
        self.assertFalse([command for command, _ in calls if command[:2] == ['xcrun', 'simctl']])
        self.assertIn('CI SMOKE handover simulator=smoke-udid to p12-ci.py', log)
        self.assertNotIn('cleanup simulator=', log)

    def test_a_failed_p9_check_deletes_the_simulator_and_hands_nothing_over(self):
        calls, log = self.smoke(p9=True, actions=True, failing='p9-check.py')
        self.assertFalse(self.handover.exists())
        self.assertTrue(is_delete(calls[-1][0]))
        self.assertIn('cleanup simulator=smoke-udid', log)

    def test_a_failed_boot_deletes_the_simulator(self):
        calls, _ = self.smoke(p9=True, actions=True, failing='-c')
        self.assertFalse(self.handover.exists())
        self.assertEqual(len(calls), 3)
        self.assertTrue(is_delete(calls[-1][0]))

    def test_outside_actions_or_without_p9_the_simulator_is_deleted(self):
        for p9, actions in ((True, False), (False, True), (False, False)):
            with self.subTest(p9=p9, actions=actions):
                calls, _ = self.smoke(p9=p9, actions=actions)
                self.assertFalse(self.handover.exists())
                self.assertTrue(is_delete(calls[-1][0]))


class P12StepTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.handover = Path(self.directory.name) / 'ci-smoke-simulator.txt'
        self.smoke = load('ci_smoke_for_p12', 'ci-smoke.py')
        self.smoke.HANDOVER = self.handover

    def p12(self, devices, uninstall_exit=0):
        calls = []
        created = []

        def run(command, **kwargs):
            calls.append(command)
            if command[:3] == ['xcrun', 'simctl', 'uninstall']:
                return subprocess.CompletedProcess(command, uninstall_exit, '', 'boom' if uninstall_exit else '')
            return ok(command)

        def output(command, **kwargs):
            if command[:3] == ['xcrun', 'simctl', 'list']:
                return json.dumps({'devices': devices})
            created.append(command)
            return 'fresh-udid\n'

        log = io.StringIO()
        with patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}), \
                patch.object(p12_ci, 'load_smoke', return_value=self.smoke, create=True), \
                patch.object(p12_ci.subprocess, 'run', side_effect=run), \
                patch.object(p12_ci.subprocess, 'check_output', side_effect=output), \
                patch.object(Path, 'mkdir'), contextlib.redirect_stdout(log):
            p12_ci.main()
        check = [command for command in calls if 'scripts/ios/p12-check.py' in command]
        self.assertEqual(len(check), 1)
        return calls, created, log.getvalue()

    def test_the_handed_over_simulator_runs_p12_after_the_app_is_uninstalled(self):
        self.handover.write_text('smoke-udid\n')
        calls, created, log = self.p12({RUNTIME: [IPAD]})
        self.assertEqual(created, [])
        self.assertFalse(self.handover.exists())
        self.assertEqual(calls[0], ['xcrun', 'simctl', 'uninstall', 'smoke-udid', 'com.shin1ohno.yebisu'])
        self.assertEqual(calls[-1], ['xcrun', 'simctl', 'delete', 'smoke-udid'])
        self.assertIn('P12 CI reusing simulator=smoke-udid', log)

    def test_p12_check_gets_the_handed_over_udid(self):
        self.handover.write_text('smoke-udid\n')
        seen = []

        def run(command, **kwargs):
            if 'scripts/ios/p12-check.py' in command:
                seen.append(kwargs['env']['HERDR_IOS_SIMULATOR'])
            return ok(command)

        with patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}), \
                patch.object(p12_ci, 'load_smoke', return_value=self.smoke, create=True), \
                patch.object(p12_ci.subprocess, 'run', side_effect=run), \
                patch.object(p12_ci.subprocess, 'check_output', return_value=json.dumps({'devices': {RUNTIME: [IPAD]}})), \
                patch.object(Path, 'mkdir'), contextlib.redirect_stdout(io.StringIO()):
            p12_ci.main()
        self.assertEqual(seen, ['smoke-udid'])

    def test_without_a_handover_p12_creates_its_own_simulator(self):
        calls, created, log = self.p12({RUNTIME: [IPAD]})
        self.assertEqual(len(created), 1)
        self.assertEqual(created[0][:4], ['xcrun', 'simctl', 'create', 'yebisu P12 CI'])
        self.assertNotIn('uninstall', [part for command in calls for part in command])
        self.assertEqual(calls[-1], ['xcrun', 'simctl', 'delete', 'fresh-udid'])
        self.assertIn('no handover', log)

    def test_a_handover_that_is_not_an_available_device_is_not_used(self):
        self.handover.write_text('gone-udid\n')
        calls, created, log = self.p12({RUNTIME: [IPAD]})
        self.assertEqual(len(created), 1)
        self.assertNotIn('gone-udid', [part for command in calls for part in command])
        self.assertEqual(calls[-1], ['xcrun', 'simctl', 'delete', 'fresh-udid'])
        self.assertIn("handover simulator='gone-udid' is not an available device", log)

    def test_a_failed_uninstall_deletes_the_handed_over_simulator_and_creates_one(self):
        self.handover.write_text('smoke-udid\n')
        calls, created, log = self.p12({RUNTIME: [IPAD]}, uninstall_exit=1)
        self.assertEqual(len(created), 1)
        self.assertIn(['xcrun', 'simctl', 'delete', 'smoke-udid'], calls)
        self.assertEqual(calls[-1], ['xcrun', 'simctl', 'delete', 'fresh-udid'])
        self.assertIn('could not uninstall com.shin1ohno.yebisu from simulator=smoke-udid (exit 1: boom)', log)

    def test_outside_actions_p12_ci_refuses_to_run(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(SystemExit, 'exclusive Mac lease'):
                p12_ci.main()


if __name__ == '__main__':
    unittest.main()
