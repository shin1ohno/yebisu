"""Credential-free tests for distribution boundaries; never call Cargo or security."""
import importlib.util
import os
import argparse
import plistlib
import tempfile
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch
from distribution_fixtures import ICON_INFO, add_icons

SPEC = importlib.util.spec_from_file_location("distribution", Path(__file__).with_name("distribution.py"))
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)


class DistributionTests(unittest.TestCase):
    def test_cache_fences_sdk_and_xcode(self):
        baseline = d.cache_key("27.2", "27B52")
        self.assertNotEqual(baseline, d.cache_key("26.2", "27B52"))
        self.assertNotEqual(baseline, d.cache_key("27.2", "different"))
        self.assertEqual(baseline, d.cache_key("27.2", "27B52"))

    def test_testflight_rejects_development_and_adhoc(self):
        helper = Mock()
        helper.entitlements.return_value = {"get-task-allow": True}
        with self.assertRaises(ValueError):
            d.distribution_entitlements({}, "testflight", helper)
        helper.entitlements.return_value = {"get-task-allow": False}
        with self.assertRaises(ValueError):
            d.distribution_entitlements({"ProvisionedDevices": ["fixture"]}, "testflight", helper)
        d.distribution_entitlements({}, "testflight", helper)
        helper.entitlements.assert_called_with({}, d.BUNDLE, d.TEAM)

    def test_adhoc_needs_devices_and_rejects_enterprise(self):
        helper = Mock()
        helper.entitlements.return_value = {"get-task-allow": False}
        for profile in ({}, {"ProvisionsAllDevices": True}, {"ProvisionedDevices": []}):
            with self.assertRaises(ValueError):
                d.distribution_entitlements(profile, "adhoc", helper)
        d.distribution_entitlements({"ProvisionedDevices": ["fixture"]}, "adhoc", helper)

    def test_missing_signing_credentials_fail_before_commands(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(d, "run") as runner:
            with self.assertRaises(ValueError):
                d.package(Mock())
            runner.assert_not_called()

    def test_plan_does_not_run_external_commands(self):
        with patch.object(sys, "argv", ["distribution.py", "plan"]), patch.object(d, "run") as runner:
            self.assertEqual(d.main(), 0)
            runner.assert_not_called()

    def test_build_refuses_shared_implicit_target(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(sys, "argv", ["distribution.py", "build"]), patch.object(subprocess, "run") as runner:
            self.assertEqual(d.main(), 1)
            runner.assert_not_called()

    def test_build_uses_yebisu_default_features_and_requires_unsigned_status(self):
        with tempfile.TemporaryDirectory() as temporary:
            app = Path(temporary) / "yebisu.app"
            app.mkdir()
            (app / "Info.plist").write_bytes(b"fixture")
            result = subprocess.CompletedProcess([], 3, stdout=str(app) + "\n")
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": temporary, "HERDR_IOS_TEAM": "UNWANTED"}), \
                    patch.object(sys, "argv", ["distribution.py", "build"]), \
                    patch.object(subprocess, "run", return_value=result) as runner, \
                    patch.object(d, "load_helper") as icons:
                self.assertEqual(d.main(), 0)
                icons.return_value.compile_icons.assert_called_once_with(app)
                command = runner.call_args.args[0]
                self.assertEqual(command, ["bash", "scripts/ios/bundle.sh", "--package", "yebisu",
                    "--bin", "yebisu", "--target", "aarch64-apple-ios", "--release",
                    "--bundle-id", "com.shin1ohno.yebisu"])
                self.assertEqual(runner.call_args.kwargs["env"]["HERDR_IOS_TEAM"], "")
                result.returncode = 1
                self.assertEqual(d.main(), 1)

    def test_sign_failure_deletes_dedicated_keychain_and_restores_search_list(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / "fixture.app"
            app.mkdir()
            (app / "fixture").write_bytes(b"fixture")
            (app / "Info.plist").write_bytes(plistlib.dumps({
                "CFBundleIdentifier": d.BUNDLE, "CFBundleSupportedPlatforms": ["iPhoneOS"],
                "CFBundleExecutable": "fixture", **ICON_INFO}))
            add_icons(app)
            secret = root / "fixture-secret"
            secret.write_bytes(b"not a real credential")
            helper = Mock()
            helper.ProfileError = RuntimeError
            helper.entitlements.return_value = {"get-task-allow": False,
                "application-identifier": f"{d.TEAM}.{d.BUNDLE}"}
            helper.decode.return_value = {}
            helper.certificate_hashes.return_value = ["fixture-identity"]
            helper.parse_identities.return_value = ["fixture-identity"]
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[0] == "codesign":
                    raise subprocess.CalledProcessError(1, command)
                return subprocess.CompletedProcess(command, 0, stdout='"/fixture/login.keychain-db"\n')

            args = argparse.Namespace(app=app, output=root / "out.ipa",
                build_number="1.2.3", method="testflight")
            with patch.dict(os.environ, {"IOS_P12_FILE": str(secret),
                    "IOS_PROFILE_FILE": str(secret), "IOS_P12_PASSWORD": "fixture-password"}), \
                    patch.object(d, "profile_helper", return_value=helper), \
                    patch.object(d, "run", side_effect=fake_run):
                with self.assertRaises(subprocess.CalledProcessError):
                    d.package(args)
            self.assertEqual(calls[-2][:2], ["security", "delete-keychain"])
            self.assertEqual(calls[-1], ["security", "list-keychains", "-d", "user",
                                        "-s", "/fixture/login.keychain-db"])
            self.assertFalse(args.output.exists())



if __name__ == "__main__":
    unittest.main()
