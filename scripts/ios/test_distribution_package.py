"""Signing lifecycle fixtures; no certificates, keychain or codesign processes."""
import argparse
import contextlib
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
import zipfile
from distribution_fixtures import ICON_INFO, SDK_INFO, add_icons
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location("distribution", Path(__file__).with_name("distribution.py"))
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)


def snapshot(root):
    return {str(file.relative_to(root)): hashlib.sha256(file.read_bytes()).hexdigest()
            for file in root.rglob("*") if file.is_file()}


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.app = self.root / "frozen-input.app"
        self.app.mkdir()
        (self.app / "fixture").write_bytes(b"original executable fixture")
        (self.app / "Info.plist").write_bytes(plistlib.dumps(dict(
            CFBundleIdentifier=d.BUNDLE, CFBundleSupportedPlatforms=["iPhoneOS"],
            CFBundleExecutable="fixture", CFBundleVersion="1", CFBundleShortVersionString="1.0", **ICON_INFO, **SDK_INFO)))
        add_icons(self.app)
        self.secret = self.root / "injected-fixture"
        self.secret.write_bytes(b"fixture bytes; no credentials")
        self.helper = Mock()
        self.helper.ProfileError = RuntimeError
        self.helper.entitlements.return_value = {"get-task-allow": False,
            "application-identifier": f"{d.TEAM}.{d.BUNDLE}"}
        self.helper.decode.return_value = {}
        self.helper.certificate_hashes.return_value = ["fixture-identity"]
        self.helper.parse_identities.return_value = ["fixture-identity"]
        self.args = argparse.Namespace(app=self.app, output=self.root / "output.ipa",
                                        build_number="2", method="testflight")
        self.calls = []
        self.failing = None
        self.collide = False

    def fake_run(self, argv, **kwargs):
        self.calls.append(argv)
        label = argv[1] if argv[0] == "security" else argv[0]
        if argv[0] == "codesign" and "--verify" in argv:
            label = "verify"
        if argv[0] == "security" and argv[1] == "list-keychains" and "-s" in argv:
            label = "restore-list"
        if label == self.failing:
            raise subprocess.CalledProcessError(1, argv)
        if argv[0] == "codesign" and "--sign" in argv:
            staged = Path(argv[-1])
            self.assertNotEqual(staged, self.app)
            self.assertEqual(plistlib.loads((staged / "Info.plist").read_bytes())["CFBundleVersion"], "2")
            self.assertEqual((staged / "embedded.mobileprovision").read_bytes(), self.secret.read_bytes())
        if argv[0] == "ditto":
            with zipfile.ZipFile(argv[-1], "w") as archive:
                for file in Path(argv[-2]).rglob("*"):
                    if file.is_file():
                        archive.write(file, file.relative_to(Path(argv[-2]).parent))
            if self.collide:
                self.args.output.write_bytes(b"other writer's output")
        return subprocess.CompletedProcess(argv, 0, stdout='"/fixture/login keychain-db"\n')

    def package(self):
        with patch.dict(os.environ, dict(IOS_P12_FILE=str(self.secret),
                IOS_PROFILE_FILE=str(self.secret), IOS_P12_PASSWORD="fixture only"), clear=True), \
                patch.object(d, "profile_helper", return_value=self.helper), \
                patch.object(d, "run", side_effect=self.fake_run):
            d.package(self.args)

    def assert_cleanup(self):
        self.assertEqual(self.calls[-2][:2], ["security", "delete-keychain"])
        self.assertEqual(self.calls[-1], ["security", "list-keychains", "-d", "user", "-s", "/fixture/login keychain-db"])
        self.assertFalse(list(self.root.glob("yebisu-ci-sign-*")))

    def test_success_stages_every_change_and_publishes_after_cleanup(self):
        original = snapshot(self.app)
        self.package()
        self.assertEqual(snapshot(self.app), original)
        self.assertEqual(d.ipa_metadata(self.args.output)["CFBundleVersion"], "2")
        with zipfile.ZipFile(self.args.output) as archive:
            self.assertIn("Payload/frozen-input.app/Assets.car", archive.namelist())
        self.assertEqual(self.args.output.stat().st_mode & 0o777, 0o600)
        self.assert_cleanup()

    def test_iconless_bundle_is_rejected_before_signing(self):
        for name in ('Assets.car', 'AppIcon76x76@2x~ipad.png'):
            file = self.app / name
            original = file.read_bytes()
            file.unlink()
            with self.subTest(missing=name), self.assertRaisesRegex(ValueError, 'AppIcon'):
                self.package()
            self.assertFalse(self.calls)
            self.assertFalse(self.args.output.exists())
            file.write_bytes(original)
        info = plistlib.loads((self.app / 'Info.plist').read_bytes())
        del info['CFBundleIcons~ipad']
        (self.app / 'Info.plist').write_bytes(plistlib.dumps(info))
        with self.assertRaisesRegex(ValueError, 'AppIcon'):
            self.package()
        self.assertFalse(self.calls)

    def test_failures_cleanup_and_do_not_publish_or_modify_source(self):
        original = snapshot(self.app)
        for label in ["create-keychain", "unlock-keychain", "import", "set-key-partition-list",
                      "find-identity", "codesign", "verify", "ditto", "delete-keychain", "restore-list"]:
            with self.subTest(stage=label):
                self.calls = []
                self.failing = label
                with self.assertRaises(subprocess.CalledProcessError):
                    self.package()
                self.assertEqual(snapshot(self.app), original)
                self.assertFalse(self.args.output.exists())
                self.assert_cleanup()

    def test_output_inside_input_and_nested_code_are_refused(self):
        original = snapshot(self.app)
        self.args.output = self.app / "out.ipa"
        with self.assertRaises(ValueError):
            self.package()
        self.assertEqual(snapshot(self.app), original)
        self.assertFalse(self.calls)
        self.args.output = self.root / "output.ipa"
        (self.app / "nested.appex").mkdir()
        with self.assertRaises(ValueError):
            self.package()
        self.assertFalse(self.calls)

    def test_symlinks_and_executable_escape_are_refused(self):
        link = self.app / "linked-secret"
        link.symlink_to(self.secret)
        with self.assertRaises(ValueError):
            self.package()
        link.unlink()
        info = plistlib.loads((self.app / "Info.plist").read_bytes())
        info["CFBundleExecutable"] = "../injected-fixture"
        (self.app / "Info.plist").write_bytes(plistlib.dumps(info))
        with self.assertRaises(ValueError):
            self.package()
        self.assertFalse(self.calls)

    def echoing_run(self, argv, **kwargs):
        """Tools echo the keychain and .p12 passwords on both streams."""
        secret = argv[argv.index("-P") + 1] if "-P" in argv else argv[argv.index("-p") + 1] if "-p" in argv else ""
        label = argv[1] if argv[0] == "security" else argv[0]
        try:
            result = self.fake_run(argv, **kwargs)
        except subprocess.CalledProcessError as error:
            raise subprocess.CalledProcessError(error.returncode, argv, f"OUT-{label} {secret}\n",
                                                f"ERR-{label} {secret}\n") from None
        if label in {"list-keychains", "find-identity"}:
            return result
        return subprocess.CompletedProcess(argv, 0, f"OUT-{label} {secret}\n", f"ERR-{label} {secret}\n")

    def package_output(self):
        stderr = io.StringIO()
        with patch.dict(os.environ, dict(IOS_P12_FILE=str(self.secret),
                IOS_PROFILE_FILE=str(self.secret), IOS_P12_PASSWORD="fixture only"), clear=True), \
                patch.object(d, "profile_helper", return_value=self.helper), \
                patch.object(d, "run", side_effect=self.echoing_run), contextlib.redirect_stderr(stderr):
            try:
                d.package(self.args)
            finally:
                self.output = stderr.getvalue()

    def assert_passwords_hidden(self):
        keychain_password = next(argv[3] for argv in self.calls if argv[:2] == ["security", "create-keychain"])
        for value in ("fixture only", keychain_password, str(self.secret)):
            self.assertNotIn(value, self.output)

    def test_success_shows_every_tool_output_without_passwords(self):
        self.package_output()
        for label in ("import", "codesign", "ditto", "delete-keychain"):
            self.assertIn(f"OUT-{label}", self.output)
            self.assertIn(f"ERR-{label}", self.output)
        self.assertIn("[security import] exit status 0", self.output)
        self.assertIn("OUT-import [redacted]", self.output)
        self.assertIn("ERR-create-keychain [redacted]", self.output)
        self.assert_passwords_hidden()

    def test_failure_shows_the_failing_tool_output_without_passwords(self):
        self.failing = "import"
        with self.assertRaises(subprocess.CalledProcessError):
            self.package_output()
        self.assertIn("[security import] exit status 1", self.output)
        self.assertIn("OUT-import [redacted]", self.output)
        self.assertIn("ERR-import [redacted]", self.output)
        self.assert_passwords_hidden()
        self.assert_cleanup()

    def test_output_collision_never_overwrites_another_writer(self):
        self.collide = True
        with self.assertRaises(FileExistsError):
            self.package()
        self.assertEqual(self.args.output.read_bytes(), b"other writer's output")
        self.assert_cleanup()


if __name__ == "__main__":
    unittest.main()
