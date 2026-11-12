"""CI adapter selects from simctl JSON and cleans up on smoke failure."""
import importlib.util
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

    def test_real_smoke_entry_point_and_cleanup_on_failure(self):
        failure = subprocess.CalledProcessError(1, 'smoke-check.py')
        with patch.object(ci_smoke.subprocess, 'check_output', side_effect=[
            json.dumps({'devices': self.devices()}), 'disposable-udid\n',
        ]) as output, patch.object(ci_smoke.subprocess, 'run', side_effect=[failure, None, None]) as run:
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
        self.assertEqual(run.call_args_list[2].args[0], ['xcrun', 'simctl', 'delete', 'disposable-udid'])
        self.assertTrue(run.call_args_list[2].kwargs['check'])


if __name__ == '__main__':
    unittest.main()
