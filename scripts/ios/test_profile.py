"""Tests for scripts/ios/profile.py: choosing a profile and deriving entitlements.

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_profile.py'

Profiles here are plain plists; a fake `security` on PATH stands in for
`security cms -D`, which would need a CMS signature.
"""

from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import textwrap
import unittest

SCRIPT = Path(__file__).with_name("profile.py")
# Loaded under another name: `profile` is also a standard library module.
SPEC = importlib.util.spec_from_file_location("ios_profile", SCRIPT)
ios_profile = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ios_profile)

TEAM = "TEAMID1234"
BUNDLE = "dev.herdr.profile-test"
APP_ID = f"{TEAM}.{BUNDLE}"
DEVICE = "00000000-0000000000000001"
CERTIFICATE = b"isolated development certificate"
OTHER_CERTIFICATE = b"another team member's certificate"
NOW = datetime(2026, 11, 1, tzinfo=timezone.utc)


def certificate_sha(certificate):
    return hashlib.sha1(certificate).hexdigest().upper()


def identities_output(*certificates):
    lines = [
        f'  {index}) {certificate_sha(certificate)} "Apple Development: Isolated ({index})"'
        for index, certificate in enumerate(certificates, 1)
    ]
    return "\n".join(lines + [f"     {len(lines)} valid identities found", ""])


def make_profile(
    app_pattern=f"{TEAM}.*",
    expires=NOW + timedelta(days=30),
    devices=(DEVICE,),
    all_devices=False,
    certificates=(CERTIFICATE,),
    name="Isolated profile",
    groups=(f"{TEAM}.*",),
    get_task_allow=True,
    platform=("iOS", "xrOS"),
):
    entitlements = {
        "application-identifier": app_pattern,
        "com.apple.developer.team-identifier": TEAM,
        "keychain-access-groups": list(groups),
        "get-task-allow": get_task_allow,
        "aps-environment": "development",
    }
    profile = {
        "Name": name,
        "UUID": name.replace(" ", "-"),
        "ApplicationIdentifierPrefix": [TEAM],
        "TeamIdentifier": [TEAM],
        "Platform": list(platform),
        "ExpirationDate": expires.replace(tzinfo=None),
        "DeveloperCertificates": list(certificates),
        "Entitlements": entitlements,
    }
    if all_devices:
        profile["ProvisionsAllDevices"] = True
    else:
        profile["ProvisionedDevices"] = list(devices)
    return profile


class SelectionTests(unittest.TestCase):
    def choose(self, profiles, device=DEVICE, identities=None):
        paths = {Path(f"{index}.mobileprovision"): profile for index, profile in enumerate(profiles)}
        reasons = []
        chosen = ios_profile.select(
            list(paths), APP_ID, device=device, identities=identities, now=NOW,
            load=paths.__getitem__, report=reasons.append,
        )
        return (None if chosen is None else paths[chosen]["Name"]), reasons

    def test_exact_app_id_wins_over_a_later_expiring_wildcard(self):
        chosen, _ = self.choose([
            make_profile(name="wildcard", expires=NOW + timedelta(days=300)),
            make_profile(app_pattern=APP_ID, name="exact"),
        ])
        self.assertEqual(chosen, "exact")

    def test_more_specific_wildcard_wins(self):
        chosen, _ = self.choose([
            make_profile(name="team"),
            make_profile(app_pattern=f"{TEAM}.dev.herdr.*", name="herdr"),
        ])
        self.assertEqual(chosen, "herdr")

    def test_latest_expiry_breaks_ties(self):
        chosen, _ = self.choose([
            make_profile(app_pattern=APP_ID, name="older", expires=NOW + timedelta(days=10)),
            make_profile(app_pattern=APP_ID, name="newer", expires=NOW + timedelta(days=90)),
        ])
        self.assertEqual(chosen, "newer")

    def test_expired_profile_is_skipped_with_its_reason(self):
        chosen, reasons = self.choose([
            make_profile(app_pattern=APP_ID, expires=NOW - timedelta(seconds=1), name="old"),
        ])
        self.assertIsNone(chosen)
        self.assertEqual(reasons, ["skipped old: expired"])

    def test_another_apps_profile_never_qualifies(self):
        chosen, reasons = self.choose([
            make_profile(app_pattern=f"{TEAM}.dev.other.app", name="other"),
            make_profile(app_pattern=f"OTHERTEAM1.{BUNDLE}", name="other team"),
        ])
        self.assertIsNone(chosen)
        self.assertEqual(len(reasons), 2)

    def test_device_must_be_listed_unless_all_devices_are_provisioned(self):
        chosen, reasons = self.choose([make_profile(devices=("someone-else",), name="theirs")])
        self.assertIsNone(chosen)
        self.assertIn("does not list the device", reasons[0])
        chosen, _ = self.choose([make_profile(devices=(), all_devices=True, name="all")])
        self.assertEqual(chosen, "all")

    def test_without_a_device_membership_is_not_checked(self):
        chosen, _ = self.choose([make_profile(devices=())], device=None)
        self.assertEqual(chosen, "Isolated profile")

    def test_certificate_must_belong_to_a_keychain_identity(self):
        identities = ios_profile.parse_identities(identities_output(CERTIFICATE))
        chosen, reasons = self.choose(
            [make_profile(certificates=(OTHER_CERTIFICATE,), name="foreign")],
            identities=identities,
        )
        self.assertIsNone(chosen)
        self.assertIn("no certificate matches", reasons[0])
        chosen, _ = self.choose(
            [make_profile(certificates=(OTHER_CERTIFICATE, CERTIFICATE))],
            identities=identities,
        )
        self.assertEqual(chosen, "Isolated profile")

    def test_non_ios_profile_is_skipped(self):
        chosen, _ = self.choose([make_profile(platform=("OSX",))])
        self.assertIsNone(chosen)

    def test_malformed_profile_is_skipped_without_ending_the_scan(self):
        odd_entitlements = make_profile(name="odd entitlements")
        odd_entitlements["Entitlements"] = ["not", "a", "dict"]
        odd_app_id = make_profile(name="odd app ID")
        odd_app_id["Entitlements"]["application-identifier"] = 42
        odd_devices = make_profile(app_pattern=APP_ID, name="odd devices")
        odd_devices["ProvisionedDevices"] = DEVICE
        chosen, reasons = self.choose([
            odd_entitlements, odd_app_id, odd_devices, make_profile(name="good"),
        ])
        self.assertEqual(chosen, "good")
        self.assertEqual(len(reasons), 3)
        self.assertTrue(all("malformed" in reason for reason in reasons), reasons)


class EntitlementTests(unittest.TestCase):
    def test_wildcard_keychain_group_narrows_to_the_apps_own(self):
        derived = ios_profile.entitlements(make_profile(), BUNDLE, TEAM, now=NOW)
        self.assertEqual(derived, {
            "application-identifier": APP_ID,
            "com.apple.developer.team-identifier": TEAM,
            "keychain-access-groups": [APP_ID],
            "get-task-allow": True,
        })

    def test_unused_and_unmatched_entitlements_are_dropped(self):
        derived = ios_profile.entitlements(
            make_profile(groups=(f"{TEAM}.dev.other.*", "com.apple.token")), BUNDLE, now=NOW
        )
        self.assertNotIn("keychain-access-groups", derived)
        self.assertNotIn("aps-environment", derived)

    def test_get_task_allow_follows_the_profile(self):
        derived = ios_profile.entitlements(make_profile(get_task_allow=False), BUNDLE, now=NOW)
        self.assertIs(derived["get-task-allow"], False)

    def test_profile_for_another_app_or_team_is_refused(self):
        with self.assertRaises(ios_profile.ProfileError):
            ios_profile.entitlements(
                make_profile(app_pattern=f"{TEAM}.dev.other"), BUNDLE, now=NOW
            )
        with self.assertRaises(ios_profile.ProfileError):
            ios_profile.entitlements(make_profile(), BUNDLE, "OTHERTEAM1", now=NOW)

    def test_expired_or_non_ios_profile_is_refused(self):
        expired = make_profile(expires=NOW - timedelta(seconds=1))
        with self.assertRaisesRegex(ios_profile.ProfileError, "expired"):
            ios_profile.entitlements(expired, BUNDLE, TEAM, now=NOW)
        with self.assertRaisesRegex(ios_profile.ProfileError, "not an iOS profile"):
            ios_profile.entitlements(make_profile(platform=("OSX",)), BUNDLE, TEAM, now=NOW)


class IdentityTests(unittest.TestCase):
    def test_parse_identities_reads_only_valid_identity_lines(self):
        text = identities_output(CERTIFICATE, OTHER_CERTIFICATE)
        self.assertEqual(
            ios_profile.parse_identities(text),
            [certificate_sha(CERTIFICATE), certificate_sha(OTHER_CERTIFICATE)],
        )

    def test_certificate_hashes_match_security_find_identity(self):
        self.assertEqual(
            ios_profile.certificate_hashes(make_profile(certificates=(CERTIFICATE,))),
            [certificate_sha(CERTIFICATE)],
        )


@unittest.skipUnless(sys.platform == "darwin", "profile.py runs where security does")
class CommandLineTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="herdr-ios-profile-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        tools = self.root / "tools"
        tools.mkdir()
        fake = tools / "security"
        fake.write_text("#!/usr/bin/env python3\n" + textwrap.dedent('''
            import sys
            args = sys.argv[1:]
            if args[:3] != ["cms", "-D", "-i"]:
                sys.exit(2)
            sys.stdout.buffer.write(open(args[3], "rb").read())
        ''').lstrip())
        fake.chmod(0o755)
        self.env = {**os.environ, "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}"}
        self.profiles = self.root / "profiles with spaces"
        self.profiles.mkdir()

    def write(self, name, profile):
        path = self.profiles / name
        path.write_bytes(plistlib.dumps(profile))
        return path

    def run_profile(self, *args, stdin=""):
        return subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPT), *args],
            env=self.env, input=stdin, text=True, capture_output=True, check=False,
        )

    def test_select_prints_the_chosen_path(self):
        self.write("a.mobileprovision", make_profile(name="wildcard"))
        exact = self.write("b.mobileprovision", make_profile(app_pattern=APP_ID, name="exact"))
        self.write("notes.txt", {"Name": "not a profile"})
        result = self.run_profile(
            "select", "--profiles-dir", str(self.profiles), "--app-id", APP_ID,
            "--device", DEVICE, "--now", "2026-11-01T00:00:00Z",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(exact))

    def test_select_fails_when_nothing_qualifies(self):
        self.write("a.mobileprovision", make_profile(expires=NOW - timedelta(days=1)))
        result = self.run_profile(
            "select", "--profiles-dir", str(self.profiles), "--profiles-dir",
            str(self.root / "missing"), "--app-id", APP_ID, "--now", "2026-11-01T00:00:00Z",
        )
        self.assertEqual(result.returncode, ios_profile.NONE_QUALIFIES)
        self.assertEqual(result.stdout, "")
        self.assertIn("expired", result.stderr)

    def test_entitlements_writes_a_narrowed_plist(self):
        source = self.write("a.mobileprovision", make_profile())
        out = self.root / "entitlements.plist"
        result = self.run_profile(
            "entitlements", "--profile", str(source), "--bundle-id", BUNDLE,
            "--team", TEAM, "--out", str(out), "--now", "2026-11-01T00:00:00Z",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(plistlib.loads(out.read_bytes())["keychain-access-groups"], [APP_ID])

    def test_identity_picks_the_keychain_identity_in_the_profile(self):
        source = self.write(
            "a.mobileprovision", make_profile(certificates=(OTHER_CERTIFICATE, CERTIFICATE))
        )
        result = self.run_profile(
            "identity", "--profile", str(source), stdin=identities_output(CERTIFICATE)
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), certificate_sha(CERTIFICATE))
        result = self.run_profile(
            "identity", "--profile", str(source), stdin=identities_output(b"unrelated")
        )
        self.assertEqual(result.returncode, 1)

    def test_certificates_lists_each_hash(self):
        source = self.write(
            "a.mobileprovision", make_profile(certificates=(CERTIFICATE, OTHER_CERTIFICATE))
        )
        result = self.run_profile("certificates", "--profile", str(source))
        self.assertEqual(
            result.stdout.split(),
            [certificate_sha(CERTIFICATE), certificate_sha(OTHER_CERTIFICATE)],
        )

    def test_select_failure_is_not_mistaken_for_nothing_installed(self):
        result = self.run_profile(
            "select", "--profiles-dir", str(self.profiles), "--app-id", APP_ID,
            "--identities", str(self.root / "missing identities.txt"),
        )
        self.assertNotIn(result.returncode, (0, ios_profile.NONE_QUALIFIES))

    def test_undecodable_profile_is_an_error_not_a_traceback(self):
        source = self.profiles / "broken.mobileprovision"
        source.write_bytes(b"not a plist")
        result = self.run_profile("entitlements", "--profile", str(source), "--bundle-id", BUNDLE)
        self.assertEqual(result.returncode, 1)
        self.assertIn("error:", result.stderr)
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
