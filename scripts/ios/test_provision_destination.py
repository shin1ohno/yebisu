"""provision.sh's opt-in --destination and --register-device, and its unchanged default.

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_provision_destination.py'

No Xcode or Apple account is involved: xcodegen, xcodebuild and security are
fakes on PATH, and xcodebuild "downloads" a plain-plist profile into Xcode's
profile directory under a temporary HOME, as Xcode's automatic signing does.
"""

from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import textwrap
import unittest

PROVISION = Path(__file__).with_name("provision.sh")
TEAM = "TEAMID1234"
BUNDLE = "com.shin1ohno.yebisu"
CERTIFICATE = b"isolated development certificate"
CERTIFICATE_SHA = hashlib.sha1(CERTIFICATE).hexdigest().upper()
MAC = "00006050-000431A83460C01C"
MAC_DESTINATION = f"platform=macOS,arch=arm64,variant=Designed for iPad,id={MAC}"
REGISTER = "-allowProvisioningDeviceRegistration"

# `provision.sh --help` on main 773e7bd, before the two options were added.
BASELINE_HELP = """\
Usage: scripts/ios/provision.sh --team TEAM --bundle-id ID [options]

Options:
  --team TEAM         Apple development team ID, ten characters
  --bundle-id ID      Bundle identifier the profile must cover
  --device UDID       Hardware UDID the profile must list
                      (xcrun devicectl list devices shows the device)
  --profiles-dir DIR  Where to look; repeatable (default: Xcode's
                      ~/Library/Developer/Xcode/UserData/Provisioning Profiles
                      and ~/Library/MobileDevice/Provisioning Profiles)
  --no-create         Fail instead of asking Xcode for a profile
  -h, --help          Show this help

Exits with status 2 for invalid options and 1 when no profile can be found
or created. Xcode is asked only when the local search ran and found nothing,
never after it failed.
"""
ADDED_HELP = [
    "  --destination DEST  xcodebuild destination for the throwaway app",
    "                      (default: generic/platform=iOS)",
    "  --register-device   Let Xcode register the destination device with the",
    "                      team (-allowProvisioningDeviceRegistration); needs",
    "                      a --destination naming one device by id=",
]


def profile_plist(devices):
    return plistlib.dumps({
        "Name": "Isolated test profile",
        "ApplicationIdentifierPrefix": [TEAM],
        "TeamIdentifier": [TEAM],
        "Platform": ["iOS"],
        "ExpirationDate": datetime.now(timezone.utc).replace(tzinfo=None) + timedelta(days=30),
        "DeveloperCertificates": [CERTIFICATE],
        "ProvisionedDevices": list(devices),
        "Entitlements": {"application-identifier": f"{TEAM}.{BUNDLE}"},
    })


FAKES = {
    "security": r'''
        import os, sys
        args = sys.argv[1:]
        if args[:3] == ["cms", "-D", "-i"]:
            sys.stdout.buffer.write(open(args[3], "rb").read())
        elif args[:1] == ["find-identity"]:
            print(f'  1) {os.environ["PROVISION_TEST_SHA"]} "Apple Development: Isolated (1)"')
            print("     1 valid identities found")
        else:
            sys.exit(2)
    ''',
    "xcodegen": "",
    # Xcode's automatic signing downloads the profile into its directory.
    "xcodebuild": r'''
        import json, os, pathlib, sys
        with open(os.environ["PROVISION_TEST_LOG"], "a") as log:
            log.write(json.dumps(sys.argv[1:]) + "\n")
        target = pathlib.Path(os.environ["PROVISION_TEST_PROFILES"]) / "created.mobileprovision"
        target.write_bytes(pathlib.Path(os.environ["PROVISION_TEST_CREATES"]).read_bytes())
    ''',
}


@unittest.skipUnless(sys.platform == "darwin", "provision.sh reads Info.plist with plutil")
class ProvisionDestinationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="herdr-ios-provision-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        tools = self.root / "tools"
        tools.mkdir()
        for name, body in FAKES.items():
            path = tools / name
            path.write_text("#!/usr/bin/env python3\n" + textwrap.dedent(body).lstrip())
            path.chmod(0o755)
        home = self.root / "home"
        # The real directory name has a space; the printed path must keep it.
        self.profiles = home / "Library/Developer/Xcode/UserData/Provisioning Profiles"
        self.profiles.mkdir(parents=True)
        self.created = self.root / "downloaded.mobileprovision"
        self.created.write_bytes(profile_plist([MAC, "00000000-000000000000000A"]))
        self.log = self.root / "xcodebuild.jsonl"
        environment = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_IOS_")}
        self.env = {
            **environment,
            "HOME": str(home),
            "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "PROVISION_TEST_LOG": str(self.log),
            "PROVISION_TEST_PROFILES": str(self.profiles),
            "PROVISION_TEST_CREATES": str(self.created),
            "PROVISION_TEST_SHA": CERTIFICATE_SHA,
        }

    def run_provision(self, *args):
        return subprocess.run(
            ["bash", str(PROVISION), "--team", TEAM, "--bundle-id", BUNDLE, *args],
            env=self.env, text=True, capture_output=True, check=False,
        )

    def xcodebuild_calls(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_default_invocation_is_unchanged(self):
        result = self.run_provision()
        self.assertEqual(result.returncode, 0, result.stderr)
        (argv,) = self.xcodebuild_calls()
        self.assertEqual(len(argv), 10, argv)
        self.assertTrue(argv[1].endswith("/stub/HerdrProvisioning.xcodeproj"), argv[1])
        self.assertTrue(argv[7].endswith("/DerivedData"), argv[7])
        golden = ["-project", argv[1], "-scheme", "HerdrProvisioning",
                  "-destination", "generic/platform=iOS", "-derivedDataPath", argv[7],
                  "-allowProvisioningUpdates", "build"]
        self.assertEqual(argv, golden)
        found = self.profiles / "created.mobileprovision"
        self.assertEqual(result.stdout.splitlines()[-1], str(found))
        app_id = f"{TEAM}.{BUNDLE}"
        self.assertEqual(result.stderr.splitlines(), [
            f"no usable provisioning profile for {app_id}",
            f"No installed profile covers {app_id}; asking Xcode to register the App ID"
            " and download one with a throwaway app",
            f"Xcode provisioned {app_id}: {found}",
        ])

    def test_default_never_registers_a_device(self):
        for extra in ((), ("--device", MAC)):
            with self.subTest(extra=extra):
                self.log.unlink(missing_ok=True)
                (self.profiles / "created.mobileprovision").unlink(missing_ok=True)
                result = self.run_provision(*extra)
                self.assertEqual(result.returncode, 0, result.stderr)
                (argv,) = self.xcodebuild_calls()
                self.assertNotIn(REGISTER, argv)
                self.assertEqual(argv[argv.index("-destination") + 1], "generic/platform=iOS")

    def test_destination_is_passed_as_one_argument(self):
        result = self.run_provision("--device", MAC, "--destination", MAC_DESTINATION)
        self.assertEqual(result.returncode, 0, result.stderr)
        (argv,) = self.xcodebuild_calls()
        self.assertEqual(argv[argv.index("-destination") + 1], MAC_DESTINATION)
        self.assertNotIn(REGISTER, argv)
        self.assertIn("Provisioning Profiles/", result.stdout.splitlines()[-1])

    def test_register_device_adds_the_flag(self):
        result = self.run_provision(
            "--device", MAC, "--destination", MAC_DESTINATION, "--register-device")
        self.assertEqual(result.returncode, 0, result.stderr)
        (argv,) = self.xcodebuild_calls()
        self.assertEqual(argv[-3:], ["-allowProvisioningUpdates", REGISTER, "build"])
        self.assertEqual(argv[argv.index("-destination") + 1], MAC_DESTINATION)

    def test_register_device_needs_an_id_destination_and_creation(self):
        message = "error: --register-device needs a --destination with id= and no --no-create"
        for args in (("--register-device",),
                     ("--destination", "generic/platform=iOS", "--register-device"),
                     ("--destination", MAC_DESTINATION, "--register-device", "--no-create")):
            with self.subTest(args=args):
                result = self.run_provision(*args)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr.strip(), message)
                self.assertEqual(self.xcodebuild_calls(), [])

    def test_help_only_adds_lines(self):
        result = subprocess.run(["bash", str(PROVISION), "--help"],
                                text=True, capture_output=True, check=False)
        self.assertEqual(result.returncode, 0)
        lines = result.stdout.splitlines()
        remaining = iter(lines)
        for line in BASELINE_HELP.splitlines():
            self.assertIn(line, remaining, "a baseline help line is missing or moved")
        added = list(lines)
        for line in BASELINE_HELP.splitlines():
            added.remove(line)
        self.assertEqual(added, ADDED_HELP)


if __name__ == "__main__":
    unittest.main()
