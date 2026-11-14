"""Exercise simulator entitlements and device signing without building an app."""

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


SCRIPT = Path(__file__).with_name("bundle.sh")
PROVISION = Path(__file__).with_name("provision.sh")
REPOSITORY = Path(__file__).resolve().parents[2]
TEAM = "TEAMID1234"
CERTIFICATE = b"isolated development certificate"
CERTIFICATE_SHA = hashlib.sha1(CERTIFICATE).hexdigest().upper()
OTHER_SHA = hashlib.sha1(b"another identity").hexdigest().upper()


DEVICE = "00000000-0000000000000001"


def profile_plist(app_pattern=f"{TEAM}.*", days=30, devices=(DEVICE,)):
    """A development profile as security cms -D prints it, for the fakes."""
    return plistlib.dumps({
        "Name": "Isolated test profile",
        "ApplicationIdentifierPrefix": [TEAM],
        "TeamIdentifier": [TEAM],
        "Platform": ["iOS"],
        "ExpirationDate": datetime.now(timezone.utc).replace(tzinfo=None) + timedelta(days=days),
        "DeveloperCertificates": [CERTIFICATE],
        "ProvisionedDevices": list(devices),
        "Entitlements": {
            "application-identifier": app_pattern,
            "com.apple.developer.team-identifier": TEAM,
            "keychain-access-groups": [f"{TEAM}.*"],
            "get-task-allow": True,
            "aps-environment": "development",
        },
    })


@unittest.skipUnless(sys.platform == "darwin", "bundling uses macOS plutil")
class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="herdr-ios-bundle-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        tools = self.root / "tools"
        tools.mkdir()
        self.executable = self.root / "build with spaces" / "yebisu"
        self.executable.parent.mkdir()
        self.executable.write_bytes(b"isolated test executable")
        self.log = self.root / "calls.jsonl"
        self.home = self.root / "home"
        self.xcode_profiles = (
            self.home / "Library/Developer/Xcode/UserData/Provisioning Profiles"
        )
        self.xcode_profiles.mkdir(parents=True)
        environment = {
            key: value for key, value in os.environ.items() if not key.startswith("HERDR_IOS_")
        }
        self.env = {
            **environment,
            "HOME": str(self.home),
            "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "HERDR_BUNDLE_TEST_LOG": str(self.log),
            "HERDR_BUNDLE_TEST_BINARY": str(self.executable),
            "HERDR_BUNDLE_TEST_PROFILES": str(self.xcode_profiles),
            "HERDR_BUNDLE_TEST_IDENTITIES": f"{OTHER_SHA} {CERTIFICATE_SHA}",
        }
        self.tool(
            tools,
            "cargo",
            r'''
            import json, os, pathlib, plistlib, sys
            args = sys.argv[1:]
            record = {"tool": "cargo", "args": args}
            for arg in args:
                if arg.startswith("link-arg=-Wl,-sectcreate,__TEXT,__entitlements,"):
                    path = pathlib.Path(arg.split(",", 4)[4])
                    record["entitlements"] = plistlib.loads(path.read_bytes())
                    record["entitlements_path"] = str(path)
            with open(os.environ["HERDR_BUNDLE_TEST_LOG"], "a") as log:
                log.write(json.dumps(record) + "\n")
            kind = "example" if "--example" in args else "bin"
            name = args[args.index("--" + kind) + 1]
            print(json.dumps({
                "reason": "compiler-artifact",
                "executable": os.environ["HERDR_BUNDLE_TEST_BINARY"],
                "target": {"name": name, "kind": [kind]},
                "package_id": "path+file:///isolated#yebisu@0.1.0",
            }))
            ''',
        )
        self.tool(
            tools,
            "codesign",
            r'''
            import json, os, pathlib, plistlib, sys
            args = sys.argv[1:]
            record = {"tool": "codesign", "args": args}
            if "--entitlements" in args:
                path = pathlib.Path(args[args.index("--entitlements") + 1])
                record["entitlements"] = plistlib.loads(path.read_bytes())
                embedded = pathlib.Path(args[-1]) / "embedded.mobileprovision"
                record["embedded"] = embedded.read_bytes().hex() if embedded.exists() else None
            with open(os.environ["HERDR_BUNDLE_TEST_LOG"], "a") as log:
                log.write(json.dumps(record) + "\n")
            ''',
        )
        # Profiles in these tests are plain plists, so `cms -D` prints them.
        self.tool(
            tools,
            "security",
            r'''
            import json, os, sys
            args = sys.argv[1:]
            with open(os.environ["HERDR_BUNDLE_TEST_LOG"], "a") as log:
                log.write(json.dumps({"tool": "security", "args": args}) + "\n")
            if args[:3] == ["cms", "-D", "-i"]:
                sys.stdout.buffer.write(open(args[3], "rb").read())
            elif args[:1] == ["find-identity"]:
                shas = os.environ["HERDR_BUNDLE_TEST_IDENTITIES"].split()
                for index, sha in enumerate(shas, 1):
                    print(f'  {index}) {sha} "Apple Development: Isolated ({index})"')
                print(f"     {len(shas)} valid identities found")
            else:
                sys.exit(2)
            ''',
        )
        self.tool(
            tools,
            "xcodegen",
            r'''
            import json, os, pathlib, sys
            args = sys.argv[1:]
            spec = pathlib.Path(args[args.index("--spec") + 1])
            record = {"tool": "xcodegen", "args": args, "spec": spec.read_text()}
            with open(os.environ["HERDR_BUNDLE_TEST_LOG"], "a") as log:
                log.write(json.dumps(record) + "\n")
            ''',
        )
        # Xcode's automatic signing downloads the profile into its directory.
        self.tool(
            tools,
            "xcodebuild",
            r'''
            import json, os, pathlib, sys
            with open(os.environ["HERDR_BUNDLE_TEST_LOG"], "a") as log:
                log.write(json.dumps({"tool": "xcodebuild", "args": sys.argv[1:]}) + "\n")
            created = os.environ.get("HERDR_BUNDLE_TEST_XCODE_CREATES")
            if created is None:
                print("error: No Account for Team")
                sys.exit(65)
            target = pathlib.Path(os.environ["HERDR_BUNDLE_TEST_PROFILES"]) / "created.mobileprovision"
            target.write_bytes(pathlib.Path(created).read_bytes())
            ''',
        )
        self.tool(
            tools,
            "rustup",
            'print("aarch64-apple-ios\\naarch64-apple-ios-sim")',
        )

    @staticmethod
    def tool(directory, name, body):
        path = directory / name
        path.write_text("#!/usr/bin/env python3\n" + textwrap.dedent(body).lstrip())
        path.chmod(0o755)

    def run_bundle(self, *args, cwd=None):
        return subprocess.run(
            ["bash", str(SCRIPT.resolve()), *args],
            cwd=cwd,
            env=self.env,
            text=True,
            capture_output=True,
            check=False,
        )

    def run_provision(self, *args):
        return subprocess.run(
            ["bash", str(PROVISION), *args],
            env=self.env,
            text=True,
            capture_output=True,
            check=False,
        )

    def calls(self, tool=None):
        if not self.log.exists():
            return []
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        return [call for call in calls if tool is None or call["tool"] == tool]

    def write_profile(self, path, **kwargs):
        path.write_bytes(profile_plist(**kwargs))
        return path

    def assert_signed_for(self, result, bundle_id, profile):
        self.assertEqual(result.returncode, 0, result.stderr)
        app = Path(result.stdout.splitlines()[-1])
        self.assertEqual(app, self.executable.parent / "yebisu.app")
        self.assertEqual((app / "embedded.mobileprovision").read_bytes(), profile.read_bytes())
        signing = [call for call in self.calls("codesign") if "--sign" in call["args"]]
        self.assertEqual(len(signing), 1)
        args = signing[0]["args"]
        self.assertEqual(args[args.index("--sign") + 1], CERTIFICATE_SHA)
        self.assertIn("--timestamp=none", args)
        self.assertEqual(signing[0]["embedded"], profile.read_bytes().hex())
        self.assertEqual(signing[0]["entitlements"], {
            "application-identifier": f"{TEAM}.{bundle_id}",
            "com.apple.developer.team-identifier": TEAM,
            "keychain-access-groups": [f"{TEAM}.{bundle_id}"],
            "get-task-allow": True,
        })
        verify = [call for call in self.calls("codesign") if "--verify" in call["args"]]
        self.assertEqual(verify[0]["args"], ["--verify", "--strict", str(app)])
        info = plistlib.loads((app / "Info.plist").read_bytes())
        self.assertEqual(info["CFBundleIdentifier"], bundle_id)
        self.assertEqual(info["CFBundleSupportedPlatforms"], ["iPhoneOS"])
        build = self.calls("cargo")[0]
        self.assertEqual(build["args"][0], "build")
        self.assertNotIn("entitlements", build)

    def test_platform_examples_do_not_bundle_the_clients_fonts(self):
        result = self.run_bundle("--example", "uikit_smoke")
        self.assertEqual(result.returncode, 0, result.stderr)
        app = self.executable.parent / "uikit_smoke.app"
        info = plistlib.loads((app / "Info.plist").read_bytes())
        self.assertNotIn("UIAppFonts", info)
        self.assertFalse((app / "Fonts").exists())

    def test_bundle_registers_sauce_code_pro_without_system_font_privileges(self):
        result = self.run_bundle("--package", "yebisu", "--bin", "yebisu")
        self.assertEqual(result.returncode, 0, result.stderr)
        app = self.executable.parent / "yebisu.app"
        info = plistlib.loads((app / "Info.plist").read_bytes())
        expected = {f"Fonts/SauceCodeProNerdFontMono-{face}.ttf"
                    for face in ["Regular", "Bold", "Italic", "BoldItalic"]}
        self.assertEqual(set(info["UIAppFonts"]), expected)
        for relative in expected:
            source = REPOSITORY / "assets/fonts/SauceCodePro" / Path(relative).name
            self.assertEqual((app / relative).read_bytes(), source.read_bytes())
        self.assertEqual((app / "Fonts/LICENSE.txt").read_bytes(),
                         (REPOSITORY / "assets/fonts/SauceCodePro/LICENSE.txt").read_bytes())
        self.assertTrue((app / "Fonts/PROVENANCE.md").is_file())
        self.assertTrue((app / "Fonts/NERD-FONTS-LICENSE.txt").is_file())
        for call in self.calls():
            self.assertNotIn("com.apple.developer.user-fonts", call.get("entitlements", {}))

    def test_collector_rejects_escaped_artifact_before_removing_app(self):
        owned = self.root / "owned"
        owned.mkdir()
        self.env["HERDR_IOS_ARTIFACT_ROOT"] = str(owned)
        app = self.executable.parent / "yebisu.app"
        app.mkdir()
        sentinel = app / "preserve"
        sentinel.write_text("old artifact")
        result = self.run_bundle("--package", "yebisu", "--bin", "yebisu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("escaped the requested isolated root", result.stderr)
        self.assertEqual(sentinel.read_text(), "old artifact")

    def test_simulator_bundle_scratch_honors_owned_tmpdir(self):
        scratch = self.root / "owned scratch"
        scratch.mkdir()
        self.env["TMPDIR"] = str(scratch)
        self.env["HERDR_IOS_ARTIFACT_ROOT"] = str(self.executable.parent)
        result = self.run_bundle("--package", "yebisu", "--bin", "yebisu")
        self.assertEqual(result.returncode, 0, result.stderr)
        call = next(c for c in self.calls("cargo") if "entitlements_path" in c)
        self.assertTrue(Path(call["entitlements_path"]).is_relative_to(scratch))
        self.assertFalse(Path(call["entitlements_path"]).exists())

    def test_simulator_links_private_keychain_entitlements_only_into_the_executable(self):
        result = self.run_bundle(
            "--package", "yebisu", "--bin", "yebisu",
            "--bundle-id", "com.shin1ohno.yebisu", "--no-default-features",
            "--features", "test-feature",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        call = self.calls()[0]
        self.assertEqual(call["args"][0], "rustc")
        self.assertIn("--locked", call["args"])
        self.assertNotIn("--manifest-path", call["args"])
        self.assertIn("--no-default-features", call["args"])
        self.assertEqual(call["args"][call["args"].index("--features") + 1], "test-feature")
        self.assertEqual(call["entitlements"], {
            "application-identifier": "HERDRSIM.com.shin1ohno.yebisu",
            "keychain-access-groups": ["HERDRSIM.com.shin1ohno.yebisu"],
        })
        self.assertFalse(Path(call["entitlements_path"]).exists())
        self.assertTrue((self.executable.parent / "yebisu.app/Info.plist").is_file())
        signatures = [call for call in self.calls() if call["tool"] == "codesign"]
        self.assertTrue(signatures)
        self.assertTrue(all("--entitlements" not in call["args"] for call in signatures))

    def test_external_manifest_is_resolved_from_the_caller_and_passed_to_cargo(self):
        manifest = self.root / "external platform" / "Cargo.toml"
        manifest.parent.mkdir()
        manifest.write_text('[package]\nname = "yebisu"\nversion = "0.1.0"\n')
        for target, subcommand, status in (
            ("aarch64-apple-ios-sim", "rustc", 0),
            ("aarch64-apple-ios", "build", 3),
        ):
            with self.subTest(target=target):
                result = self.run_bundle(
                    "--manifest-path", "external platform/Cargo.toml",
                    "--example", "hello", "--target", target, cwd=self.root,
                )
                self.assertEqual(result.returncode, status, result.stderr)
                args = self.calls("cargo")[-1]["args"]
                self.assertEqual(args[0], subcommand)
                self.assertEqual(args[args.index("--manifest-path") + 1], str(manifest.resolve()))
                self.assertEqual(args[args.index("--package") + 1], "yebisu")
                self.assertEqual(args[args.index("--example") + 1], "hello")
                self.assertTrue((self.executable.parent / "hello.app/Info.plist").is_file())

    def test_invalid_manifest_options_exit_before_invoking_any_build_tool(self):
        for args in (("--manifest-path",), ("--manifest-path", str(self.root / "missing.toml"))):
            with self.subTest(args=args):
                result = self.run_bundle("--example", "hello", *args)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(self.calls())

    def test_device_without_a_team_stays_unsigned_and_names_the_options(self):
        result = self.run_bundle("--bin", "yebisu", "--target", "aarch64-apple-ios")
        self.assertEqual(result.returncode, 3, result.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0]["args"][0], "build")
        self.assertNotIn("entitlements", calls[0])
        self.assertIn("unsigned device bundle", result.stderr)
        self.assertIn("--team", result.stderr)
        self.assertIn("--profile", result.stderr)
        self.assertFalse((self.executable.parent / "yebisu.app/embedded.mobileprovision").exists())

    def test_device_with_a_profile_signs_with_the_identity_the_profile_lists(self):
        profile = self.write_profile(self.root / "given profile.mobileprovision")
        result = self.run_bundle(
            "--package", "yebisu", "--bin", "yebisu", "--target", "aarch64-apple-ios",
            "--bundle-id", "com.shin1ohno.yebisu", "--team", TEAM, "--profile", str(profile),
        )
        self.assert_signed_for(result, "com.shin1ohno.yebisu", profile)
        self.assertFalse(self.calls("xcodebuild"))

    def test_team_from_the_environment_finds_xcodes_installed_profile(self):
        profile = self.write_profile(
            self.xcode_profiles / "installed.mobileprovision", app_pattern=f"{TEAM}.com.shin1ohno.yebisu"
        )
        self.env["HERDR_IOS_TEAM"] = TEAM
        self.env["HERDR_IOS_BUNDLE_ID"] = "com.shin1ohno.yebisu"
        result = self.run_bundle("--bin", "yebisu", "--target", "aarch64-apple-ios")
        self.assert_signed_for(result, "com.shin1ohno.yebisu", profile)
        self.assertFalse(self.calls("xcodebuild"))

    def test_missing_profile_is_created_by_xcode_before_the_build(self):
        created = self.write_profile(self.root / "xcode would download.mobileprovision")
        self.env["HERDR_BUNDLE_TEST_XCODE_CREATES"] = str(created)
        scratch = self.root / "scratch"
        scratch.mkdir()
        self.env["TMPDIR"] = str(scratch)
        result = self.run_bundle(
            "--bin", "yebisu", "--target", "aarch64-apple-ios", "--release",
            "--bundle-id", "com.shin1ohno.yebisu", "--team", TEAM,
        )
        self.assert_signed_for(result, "com.shin1ohno.yebisu", created)
        spec = self.calls("xcodegen")[0]["spec"]
        self.assertIn('PRODUCT_BUNDLE_IDENTIFIER: "com.shin1ohno.yebisu"', spec)
        self.assertIn(f'DEVELOPMENT_TEAM: "{TEAM}"', spec)
        self.assertIn("CODE_SIGN_STYLE: Automatic", spec)
        generate = self.calls("xcodegen")[0]["args"]
        self.assertIn("--no-env", generate)
        spec_path = Path(generate[generate.index("--spec") + 1]).resolve()
        args = self.calls("xcodebuild")[0]["args"]
        derived = Path(args[args.index("-derivedDataPath") + 1]).resolve()
        for path in (spec_path, derived):
            self.assertTrue(path.is_relative_to(scratch.resolve()), path)
            self.assertFalse(path.is_relative_to(REPOSITORY), path)
        self.assertFalse(spec_path.exists(), "the stub outlives provision.sh")
        self.assertIn("-allowProvisioningUpdates", args)
        self.assertEqual(args[args.index("-destination") + 1], "generic/platform=iOS")
        tools = [call["tool"] for call in self.calls()]
        self.assertLess(tools.index("xcodebuild"), tools.index("cargo"))
        self.assertNotIn(TEAM, result.stdout.splitlines()[-1])

    def test_unprovisionable_team_fails_before_compiling(self):
        result = self.run_bundle(
            "--bin", "yebisu", "--target", "aarch64-apple-ios", "--team", TEAM,
            "--bundle-id", "com.shin1ohno.yebisu",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Sign in to Xcode", result.stderr)
        self.assertFalse(self.calls("cargo"))
        self.assertFalse(self.calls("codesign"))

    def test_profile_without_a_keychain_identity_fails_before_compiling(self):
        profile = self.write_profile(self.root / "given.mobileprovision")
        self.env["HERDR_BUNDLE_TEST_IDENTITIES"] = OTHER_SHA
        result = self.run_bundle(
            "--bin", "yebisu", "--target", "aarch64-apple-ios", "--profile", str(profile),
            "--bundle-id", "com.shin1ohno.yebisu",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no signing identity", result.stderr)
        self.assertFalse(self.calls("cargo"))

    def test_expired_profile_given_explicitly_fails_before_compiling(self):
        profile = self.write_profile(self.root / "expired.mobileprovision", days=-1)
        result = self.run_bundle(
            "--bin", "yebisu", "--target", "aarch64-apple-ios", "--profile", str(profile),
            "--bundle-id", "com.shin1ohno.yebisu",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("expired", result.stderr)
        self.assertFalse(self.calls("cargo"))
        self.assertFalse(self.calls("codesign"))

    def test_rejects_bundle_identifier_override_before_any_tool(self):
        result = self.run_bundle("--bin", "yebisu", "--bundle-id", "com.example.other")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("bundle identifier is fixed", result.stderr)
        self.assertFalse(self.log.exists())

    def test_simulator_keeps_the_projects_bundle_id_whatever_the_environment(self):
        self.env["HERDR_IOS_BUNDLE_ID"] = "com.example.old-app"
        result = self.run_bundle("--bin", "yebisu")
        self.assertEqual(result.returncode, 0, result.stderr)
        info = plistlib.loads((self.executable.parent / "yebisu.app/Info.plist").read_bytes())
        self.assertEqual(info["CFBundleIdentifier"], "com.shin1ohno.yebisu")
        self.assertEqual(info["CFBundleName"], "yebisu")
        self.assertEqual(info["NSLocalNetworkUsageDescription"], "Connect to your Herdr hosts.")
        self.assertNotIn("CFBundleURLTypes", info)
        self.assertEqual(
            self.calls("cargo")[0]["entitlements"]["application-identifier"],
            "HERDRSIM.com.shin1ohno.yebisu",
        )

    def test_provision_refuses_values_that_could_escape_the_stub_spec(self):
        for args in (
            ("--team", TEAM, "--bundle-id", 'a"\n  OTHER: x'),
            ("--team", TEAM, "--bundle-id", "${HOME}"),
            ("--team", TEAM, "--bundle-id", "a b"),
            ("--team", "abc", "--bundle-id", "com.shin1ohno.yebisu"),
            ("--team", f'{TEAM}"', "--bundle-id", "com.shin1ohno.yebisu"),
        ):
            result = self.run_provision(*args)
            self.assertEqual(result.returncode, 2, (args, result.stderr))
        self.assertFalse(self.log.exists())

    def test_provision_never_asks_xcode_after_a_failed_search(self):
        created = self.write_profile(self.root / "xcode would download.mobileprovision")
        self.env["HERDR_BUNDLE_TEST_XCODE_CREATES"] = str(created)
        unreadable = self.root / "unreadable profiles"
        unreadable.mkdir()
        unreadable.chmod(0)
        self.addCleanup(unreadable.chmod, 0o700)
        result = self.run_provision(
            "--team", TEAM, "--bundle-id", "com.shin1ohno.yebisu", "--profiles-dir", str(unreadable),
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("searching the installed provisioning profiles failed", result.stderr)
        self.assertFalse(self.calls("xcodegen"))
        self.assertFalse(self.calls("xcodebuild"))

    def test_provision_names_a_profile_that_predates_the_device(self):
        stale = self.write_profile(
            self.xcode_profiles / "stale.mobileprovision",
            app_pattern=f"{TEAM}.com.shin1ohno.yebisu", devices=("someone-else",),
        )
        self.env["HERDR_BUNDLE_TEST_XCODE_CREATES"] = str(stale)
        result = self.run_provision(
            "--team", TEAM, "--bundle-id", "com.shin1ohno.yebisu", "--device", DEVICE,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("does not list device", result.stderr)
        self.assertIn("delete that profile", result.stderr)
        self.assertEqual(len(self.calls("xcodebuild")), 1)

    def test_signing_options_are_refused_for_the_simulator(self):
        result = self.run_bundle("--bin", "yebisu", "--team", TEAM)
        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())

    def test_team_in_the_environment_leaves_the_simulator_ad_hoc(self):
        self.env["HERDR_IOS_TEAM"] = TEAM
        result = self.run_bundle("--bin", "yebisu")
        self.assertEqual(result.returncode, 0, result.stderr)
        signing = [call for call in self.calls("codesign") if "--sign" in call["args"]]
        self.assertEqual(signing[0]["args"][signing[0]["args"].index("--sign") + 1], "-")
        self.assertFalse(self.calls("security"))

    def test_bundle_declares_local_network_access_and_only_ssh_discovery(self):
        result = self.run_bundle("--package", "yebisu", "--bin", "yebisu")
        self.assertEqual(result.returncode, 0, result.stderr)
        info = plistlib.loads(
            (self.executable.parent / "yebisu.app/Info.plist").read_bytes()
        )
        self.assertTrue(info["NSLocalNetworkUsageDescription"].strip())
        self.assertEqual(info["NSBonjourServices"], ["_ssh._tcp"])

    def test_bundle_does_not_claim_the_old_apps_url_scheme(self):
        info = plistlib.loads((REPOSITORY / "scripts/ios/Info.plist").read_bytes())
        self.assertNotIn("CFBundleURLTypes", info)

    def test_invalid_target_exits_before_invoking_any_build_tool(self):
        result = self.run_bundle("--bin", "yebisu", "--target", "invalid")
        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
