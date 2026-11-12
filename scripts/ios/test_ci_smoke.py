"""CI adapter selects from simctl JSON and cleans up on success and failure."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('ci_smoke', Path(__file__).with_name('ci-smoke.py'))
ci_smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci_smoke)


class CISmokeTests(unittest.TestCase):
    def devices(self):
        # Shape from simctl list --json devices available on Xcode 26.
        ipad = {'name': 'iPad Pro 13-inch (M4)', 'isAvailable': True,
                'deviceTypeIdentifier': 'com.apple.CoreSimulator.SimDeviceType.iPad-Pro-13-inch-M4-16GB'}
        return {
            'com.apple.CoreSimulator.SimRuntime.iOS-26-0': [ipad],
            'com.apple.CoreSimulator.SimRuntime.iOS-26-5': [ipad],
            'com.apple.CoreSimulator.SimRuntime.watchOS-26-5': [ipad],
        }

    def test_newest_available_ipad_template(self):
        device_type, runtime = ci_smoke.select_template(self.devices())
        self.assertEqual(runtime, 'com.apple.CoreSimulator.SimRuntime.iOS-26-5')
        self.assertIn('iPad-Pro-13-inch', device_type)
        with self.assertRaisesRegex(RuntimeError, 'No available iPad'):
            ci_smoke.select_template({'com.apple.CoreSimulator.SimRuntime.iOS-26-5': [
                {'name': 'iPhone 17', 'isAvailable': True},
                {'name': 'iPad Pro 13-inch (M4)', 'isAvailable': False},
            ]})

    def test_runtime_versions_are_numeric(self):
        for older in ('26-5', '9-3'):
            with self.subTest(older=older):
                ipad = self.devices()['com.apple.CoreSimulator.SimRuntime.iOS-26-5']
                devices = {f'com.apple.CoreSimulator.SimRuntime.iOS-{version}': ipad
                           for version in (older, '26-10')}
                self.assertEqual(ci_smoke.select_template(devices)[1],
                                 'com.apple.CoreSimulator.SimRuntime.iOS-26-10')

    def test_newer_runtime_without_available_ipad_is_not_selected(self):
        devices = self.devices()
        devices['com.apple.CoreSimulator.SimRuntime.iOS-27-0'] = [
            {'name': 'iPhone 17', 'isAvailable': True},
            {'name': 'iPad Pro 13-inch (M4)', 'isAvailable': False},
        ]
        self.assertEqual(ci_smoke.select_template(devices)[1],
                         'com.apple.CoreSimulator.SimRuntime.iOS-26-5')

    def test_non_numeric_runtime_is_skipped_with_message(self):
        devices = self.devices()
        runtime = 'com.apple.CoreSimulator.SimRuntime.iOS-27-0-beta'
        devices[runtime] = devices['com.apple.CoreSimulator.SimRuntime.iOS-26-5']
        log = io.StringIO()
        with contextlib.redirect_stdout(log):
            self.assertEqual(ci_smoke.select_template(devices)[1],
                             'com.apple.CoreSimulator.SimRuntime.iOS-26-5')
        self.assertIn(f'skipping runtime={runtime}: non-numeric iOS version', log.getvalue())
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, 'No available iPad'):
                ci_smoke.select_template({runtime: devices[runtime]})

    def test_cleanup_after_success_and_failed_shutdown(self):
        for shutdown_code in (0, 1):
            with self.subTest(shutdown_code=shutdown_code):
                calls = []

                def simulate_run(command, *, check, **kwargs):
                    calls.append((command, check))
                    code = shutdown_code if command[:3] == ['xcrun', 'simctl', 'shutdown'] else 0
                    # Model subprocess.run: check=True raises on failed shutdown.
                    if check and code:
                        raise subprocess.CalledProcessError(code, command)
                    return subprocess.CompletedProcess(command, code)

                log = io.StringIO()
                with patch.object(ci_smoke.subprocess, 'check_output', side_effect=[
                    json.dumps({'devices': self.devices()}), 'disposable-udid\n',
                ]), patch.object(ci_smoke.subprocess, 'run', side_effect=simulate_run), \
                        contextlib.redirect_stdout(log):
                    ci_smoke.main()
                self.assertEqual(len(calls), 3)
                self.assertEqual(calls[1], (['xcrun', 'simctl', 'shutdown', 'disposable-udid'], False))
                self.assertEqual(calls[2], (['xcrun', 'simctl', 'delete', 'disposable-udid'], True))
                self.assertIn(f'cleanup simulator=disposable-udid shutdown_exit={shutdown_code} deleted',
                              log.getvalue())

    def test_delete_failure_is_not_reported_as_success(self):
        log = io.StringIO()
        with patch.object(ci_smoke.subprocess, 'check_output', side_effect=[
            json.dumps({'devices': self.devices()}), 'disposable-udid\n',
        ]), patch.object(ci_smoke.subprocess, 'run', side_effect=[
            subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], 0),
            subprocess.CalledProcessError(1, 'delete'),
        ]), contextlib.redirect_stdout(log):
            with self.assertRaises(subprocess.CalledProcessError):
                ci_smoke.main()
        self.assertNotIn('cleanup simulator=', log.getvalue())

    def test_real_smoke_entry_point_and_cleanup_on_failure(self):
        failure = subprocess.CalledProcessError(1, 'smoke-check.py')
        with patch.object(ci_smoke.subprocess, 'check_output', side_effect=[
            json.dumps({'devices': self.devices()}), 'disposable-udid\n',
        ]) as output, patch.object(ci_smoke.subprocess, 'run', side_effect=[
            failure, subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], 0),
        ]) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                ci_smoke.main()
        self.assertEqual(output.call_args_list[1].args[0], [
            'xcrun', 'simctl', 'create', 'yebisu CI smoke',
            'com.apple.CoreSimulator.SimDeviceType.iPad-Pro-13-inch-M4-16GB',
            'com.apple.CoreSimulator.SimRuntime.iOS-26-5',
        ])
        invocation = run.call_args_list[0]
        self.assertEqual(Path(invocation.args[0][1]).name, 'smoke-check.py')
        self.assertEqual(invocation.kwargs['env']['HERDR_IOS_SIMULATOR'], 'disposable-udid')
        self.assertTrue(invocation.kwargs['check'])
        self.assertEqual(run.call_args_list[1].args[0], ['xcrun', 'simctl', 'shutdown', 'disposable-udid'])
        self.assertFalse(run.call_args_list[1].kwargs['check'])
        self.assertEqual(run.call_args_list[2].args[0], ['xcrun', 'simctl', 'delete', 'disposable-udid'])
        self.assertTrue(run.call_args_list[2].kwargs['check'])


if __name__ == '__main__':
    unittest.main()
