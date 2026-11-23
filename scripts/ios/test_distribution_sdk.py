"""App Store Connect bundle metadata: Xcode SDK keys and the arm64 requirement.

The Xcode probes are faked, so these run without Xcode. test_distribution_icons.py
stamps a bundle with the real probes on macOS.
"""
import argparse
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
import zipfile
from distribution_fixtures import ICON_FILES, ICON_INFO, PROBE_OUTPUT, SDK_INFO, add_icons

SPEC = importlib.util.spec_from_file_location("distribution", Path(__file__).with_name("distribution.py"))
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)

SENTINEL = "SENTINEL-27.2-PRIVATE"
BUNDLE_SH = ["bash", "scripts/ios/bundle.sh"]


class Tools:
    """bundle.sh and the Xcode probes, answered from fixtures; records every command."""

    def __init__(self, app, outputs=None, failing=None, missing=None):
        self.app, self.calls = app, []
        self.outputs = {**PROBE_OUTPUT, **(outputs or {})}
        self.failing, self.missing = failing, missing

    def __call__(self, argv, **kwargs):
        self.calls.append(list(argv))
        if argv[:2] == BUNDLE_SH:
            return subprocess.CompletedProcess(argv, 3, stdout=f"{self.app}\n")
        key = tuple(argv)
        if key == self.missing:
            raise FileNotFoundError(2, "No such file or directory", argv[0])
        if key == self.failing:
            # run() passes check=True, so a failing probe raises like subprocess.run.
            raise subprocess.CalledProcessError(1, argv, SENTINEL, SENTINEL)
        return subprocess.CompletedProcess(argv, 0, stdout=self.outputs[key], stderr="")


class SdkMetadataTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.app = self.root / "yebisu.app"
        self.app.mkdir()
        template = plistlib.loads((d.ROOT / "scripts/ios/Info.plist").read_bytes())
        (self.app / "Info.plist").write_bytes(plistlib.dumps(template))

    def build(self, tools):
        """Run the build stage; the icon check records the Info.plist it was handed."""
        seen = []
        icons = Mock()
        icons.check_bundle.side_effect = lambda app: seen.append(
            plistlib.loads((app / "Info.plist").read_bytes()))
        stderr = io.StringIO()
        with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.root)}), \
                patch.object(sys, "argv", ["distribution.py", "build"]), \
                patch.object(subprocess, "run", side_effect=tools), \
                patch.object(d, "load_helper", return_value=icons), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
            code = d.main()
        return code, seen, stderr.getvalue()

    def test_info_template_requires_arm64(self):
        template = plistlib.loads((d.ROOT / "scripts/ios/Info.plist").read_bytes())
        self.assertEqual(template["UIRequiredDeviceCapabilities"], ["arm64"])

    def test_build_stamps_each_key_from_the_xcode_in_use_before_icons(self):
        tools = Tools(self.app)
        code, seen, stderr = self.build(tools)
        self.assertEqual(code, 0, stderr)
        self.assertEqual(len(seen), 1, "the icon is checked once, after stamping")
        self.assertEqual({key: seen[0].get(key) for key in SDK_INFO}, SDK_INFO)
        self.assertEqual(plistlib.loads((self.app / "Info.plist").read_bytes())["DTXcode"], "2720")
        bundle = next(index for index, argv in enumerate(tools.calls) if argv[:2] == BUNDLE_SH)
        self.assertEqual({tuple(argv) for argv in tools.calls[:bundle]}, set(PROBE_OUTPUT),
                         "every probe runs before the long build")

    def test_xcode_version_becomes_four_digits(self):
        for version, build, expected in [("27.2", "27B52", "2720"), ("16.2", "16C5032a", "1620"),
                                         ("15.4.1", "15F31d", "1541"), ("9.4", "9F1027a", "0940")]:
            tools = Tools(self.app, {("xcodebuild", "-version"): f"Xcode {version}\nBuild version {build}\n"})
            with self.subTest(version=version), patch.object(subprocess, "run", side_effect=tools):
                metadata = d.sdk_metadata()
                self.assertEqual(metadata["DTXcode"], expected)
                self.assertEqual(metadata["DTXcodeBuild"], build)

    def test_xcode_version_forms_and_build_formats(self):
        # Builds as listed by xcodereleases.com/data.json; "Xcode N" is accepted although recent
        # releases print "N.0".
        for first, build, expected in [("Xcode 27", "27A266a", "2700"), ("Xcode 27.0", "27A266a", "2700"),
                                       ("Xcode 27.2.1", "27B52", "2721"), ("Xcode 16.4", "16F6", "1640"),
                                       ("Xcode 14.3.1", "14E300c", "1431"), ("Xcode 26.4.1", "17E202", "2641")]:
            tools = Tools(self.app, {("xcodebuild", "-version"): f"{first}\nBuild version {build}\n"})
            with self.subTest(first=first), patch.object(subprocess, "run", side_effect=tools):
                metadata = d.sdk_metadata()
                self.assertEqual((metadata["DTXcode"], metadata["DTXcodeBuild"]), (expected, build))

    def test_unrecognised_xcode_version_lines_fail_closed(self):
        # No Xcode release has a two-digit minor or patch, and a beta suffix is not known to be printed.
        for first, build in [("Xcode 27.10", "27B52"), ("Xcode 27.2.10", "27B52"), ("Xcode 27.2 Beta", "27B52"),
                             ("Xcode 27.", "27B52"), ("Xcode", "27B52"), ("Xcode 270.1", "27B52"),
                             ("Xcode 27.2.1.1", "27B52"), ("Xcode 27.2", "27B"), ("Xcode 27.2", "B52")]:
            tools = Tools(self.app, {("xcodebuild", "-version"): f"{first}\nBuild version {build}\n"})
            with self.subTest(first=first, build=build), patch.object(subprocess, "run", side_effect=tools):
                with self.assertRaisesRegex(ValueError, "xcodebuild -version printed an unexpected value"):
                    d.sdk_metadata()

    def test_probe_failure_or_unexpected_output_stops_before_the_build(self):
        unexpected = {("xcrun", "--sdk", "iphoneos", "--show-sdk-version"): f"{SENTINEL}\n",
                      ("xcrun", "--sdk", "iphoneos", "--show-sdk-build-version"): f"{SENTINEL}\n",
                      ("xcodebuild", "-version"): f"Xcode 27.2\nBuild version 27B52\n{SENTINEL}\n",
                      ("sw_vers", "-buildVersion"): f"26B5091g {SENTINEL}\n"}
        for key in PROBE_OUTPUT:
            label = " ".join(key)
            cases = {"exit status": (Tools(self.app, failing=key), f"error: {label} failed"),
                     "missing tool": (Tools(self.app, missing=key), f"error: {label} failed"),
                     "unexpected shape": (Tools(self.app, {key: unexpected[key]}),
                                          f"error: {label} printed an unexpected value")}
            for name, (tools, message) in cases.items():
                with self.subTest(probe=label, case=name):
                    code, seen, stderr = self.build(tools)
                    self.assertEqual(code, 1)
                    self.assertIn(message, stderr)
                    self.assertNotIn(SENTINEL, stderr)
                    self.assertFalse([argv for argv in tools.calls if argv[:2] == BUNDLE_SH])
                    self.assertFalse(seen)
                    self.assertNotIn("DTPlatformName", plistlib.loads((self.app / "Info.plist").read_bytes()))


def refused(info):
    """Each way a bundle lacks what App Store Connect requires."""
    for key in SDK_INFO:
        yield f"without {key}", {name: value for name, value in info.items() if name != key}
    yield "simulator platform", dict(info, DTPlatformName="iphonesimulator")
    yield "three-digit DTXcode", dict(info, DTXcode="272")
    yield "no arm64", dict(info, UIRequiredDeviceCapabilities=["armv7"])
    yield "arm64 not required", dict(info, UIRequiredDeviceCapabilities={"arm64": False})


REFUSAL = "App Store Connect|iphoneos build"


class StoreMetadataRefusalTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.info = dict(CFBundleIdentifier=d.BUNDLE, CFBundleSupportedPlatforms=["iPhoneOS"],
                         CFBundleExecutable="fixture", CFBundleVersion="1",
                         CFBundleShortVersionString="1.0", **ICON_INFO, **SDK_INFO)

    def test_complete_metadata_passes_in_either_capability_form(self):
        d.require_store_metadata(self.info)
        d.require_store_metadata(dict(self.info, UIRequiredDeviceCapabilities={"arm64": True}))

    def test_refusal_names_the_keys_and_never_their_values(self):
        for name, info, keys in [
                ("missing", {k: v for k, v in self.info.items() if k not in ("DTXcode", "DTSDKName")},
                 ["DTSDKName", "DTXcode"]),
                ("platform", dict(self.info, DTPlatformName="iphonesimulator"), ["DTPlatformName"]),
                ("sdk", dict(self.info, DTSDKName="iphoneos0.0"), ["DTSDKName"]),
                ("xcode", dict(self.info, DTXcode="272"), ["DTXcode"]),
                ("several", dict(self.info, DTPlatformName="x", DTXcode="y"), ["DTPlatformName", "DTXcode"]),
                ("capabilities", dict(self.info, UIRequiredDeviceCapabilities=["armv7"]),
                 ["UIRequiredDeviceCapabilities"])]:
            with self.subTest(name), self.assertRaises(ValueError) as caught:
                d.require_store_metadata(info)
            message = str(caught.exception)
            for key in keys:
                self.assertIn(key, message)
            for value in ("iphonesimulator", "iphoneos0.0", "272", "armv7"):
                self.assertNotIn(value, message)

    def test_package_refuses_before_signing(self):
        app = self.root / "fixture.app"
        app.mkdir()
        (app / "fixture").write_bytes(b"fixture executable")
        add_icons(app)
        secret = self.root / "fixture-secret"
        secret.write_bytes(b"not a real credential")
        helper = Mock()
        helper.ProfileError = RuntimeError
        helper.decode.return_value = {}
        helper.entitlements.return_value = {"get-task-allow": False,
                                            "application-identifier": f"{d.TEAM}.{d.BUNDLE}"}
        args = argparse.Namespace(app=app, output=self.root / "out.ipa", build_number="1", method="testflight")
        for name, info in refused(self.info):
            (app / "Info.plist").write_bytes(plistlib.dumps(info))
            with self.subTest(name), patch.dict(os.environ, {"IOS_P12_FILE": str(secret),
                    "IOS_PROFILE_FILE": str(secret), "IOS_P12_PASSWORD": "fixture"}, clear=True), \
                    patch.object(d, "profile_helper", return_value=helper), \
                    patch.object(d, "run", side_effect=AssertionError("signing started")) as runner:
                with self.assertRaisesRegex(ValueError, REFUSAL):
                    d.package(args)
                runner.assert_not_called()
                self.assertFalse(args.output.exists())

    def test_upload_metadata_check_refuses_before_transport(self):
        ipa = self.root / "fixture.ipa"
        for name, info in refused(self.info):
            with zipfile.ZipFile(ipa, "w") as archive:
                archive.writestr("Payload/fixture.app/Info.plist", plistlib.dumps(info))
                for file, content in ICON_FILES.items():
                    archive.writestr("Payload/fixture.app/" + file, content)
            with self.subTest(name), patch.dict(os.environ, {}, clear=True), \
                    patch.object(d, "run", side_effect=AssertionError("transport started")) as runner:
                with self.assertRaisesRegex(ValueError, REFUSAL):
                    d.ipa_metadata(ipa)
                with self.assertRaisesRegex(ValueError, REFUSAL):
                    d.upload_ipa(ipa)
                runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
