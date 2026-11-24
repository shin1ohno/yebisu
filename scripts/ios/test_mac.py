"""Tests for scripts/ios/mac.sh and mac.py against fake macOS tools.

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_mac.py'

No Xcode, Apple account, build or real app is involved: provision.sh and
bundle.sh are stubs in a copied tree, and uname, system_profiler, security,
xcrun, codesign, open and ps are fakes on PATH. The fake ps lists only the
`sleep` processes these tests start, so a real yebisu is never seen or signalled.
Output shapes follow real samples taken on the target Mac (system_profiler,
security find-identity, codesign --xml, ps -o pid=,ppid=,comm=).
"""

from contextlib import redirect_stderr, redirect_stdout
from datetime import datetime, timedelta, timezone
import importlib.util
import io
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import subprocess
import sys
import tempfile
import textwrap
import unittest

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
SPEC = importlib.util.spec_from_file_location("mac", HERE / "mac.py")
mac = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mac)

BUNDLE = "com.shin1ohno.yebisu"
TEAM = "AL9567565U"
MAC = "00006050-000431A83460C01C"
OTHER_DEVICE = "00008103-000000000000000A"
DESTINATION = f"platform=macOS,arch=arm64,variant=Designed for iPad,id={MAC}"
LOG_SOURCE = (REPOSITORY / "crates/yebisu/src/log-location.txt").read_text().strip()
HARDWARE = textwrap.dedent(f"""\
    Hardware:

        Hardware Overview:

          Model Name: MacBook Pro
          Model Identifier: Mac17,9
          Serial Number (system): DGYK09TW31
          Provisioning UDID: {MAC}
          Activation Lock Status: Disabled
    """)
IDENTITY = '89A06C3BA3E11E1676DA2CEF0A187F1B670838D4 "Apple Development: Isolated (UV2CS6636M)"'
VALID = f"  1) {IDENTITY}\n     1 valid identities found\n"
NONE_VALID = "     0 valid identities found\n"
# Without -v. The trailing reason of an untrusted identity is assumed (not
# observed: the target Mac's identity is trusted); the parser ignores it.
UNTRUSTED = (f"\nPolicy: Code Signing\n  Matching identities\n  1) {IDENTITY} (CSSMERR_TP_NOT_TRUSTED)\n"
             "     1 identities found\n\n  Valid identities only\n     0 valid identities found\n")
NO_IDENTITY = ("\nPolicy: Code Signing\n  Matching identities\n     0 identities found\n\n"
               "  Valid identities only\n     0 valid identities found\n")
# Xcode's line for an unregistered Mac (task 84b82ca87da14acb, 02-stub-build.log).
UNREGISTERED = ('error: Device "DGYK09TW31" isn\'t registered in your developer account. The device'
                " must be registered in order to be included in a provisioning profile.")

PRELUDE = r'''
import json, os, pathlib, plistlib, shutil, subprocess, sys, tempfile
args = sys.argv[1:]
env = os.environ.get
def log(tool):
    with open(os.environ["MAC_TEST_LOG"], "a") as calls:
        calls.write(json.dumps({"tool": tool, "args": args}) + "\n")
def registry(entries=None):
    path = pathlib.Path(os.environ["MAC_TEST_PROCESSES"])
    if entries is None:
        return json.loads(path.read_text())
    path.write_text(json.dumps(entries))
def spawn():
    started = subprocess.run(["/bin/sh", "-c", "sleep 600 </dev/null >/dev/null 2>&1 & echo $!"],
                             capture_output=True, text=True, check=True)
    return int(started.stdout)
'''
FAKES = {
    "uname": 'log("uname"); print(env("MAC_TEST_ARCH", "arm64"))',
    "system_profiler": r'''
        log("system_profiler")
        print("system_profiler[1:2] hw.cpufamily: 0xf76c5b1a", file=sys.stderr)
        sys.stdout.write(env("MAC_TEST_HARDWARE"))
    ''',
    "security": r'''
        log("security")
        if args[:3] == ["cms", "-D", "-i"]:
            sys.stdout.buffer.write(pathlib.Path(args[3]).read_bytes())
        elif args[:1] == ["find-identity"]:
            sys.stdout.write(env("MAC_TEST_VALID") if "-v" in args else env("MAC_TEST_LISTED"))
        else:
            sys.exit(2)
    ''',
    "xcrun": 'log("xcrun"); sys.exit(1) if env("MAC_TEST_NO_SDK") else print("27.2")',
    "codesign": r'''
        log("codesign")
        if "--verify" in args:
            sys.exit(int(env("MAC_TEST_VERIFY_STATUS", "0")))
        print(f"Executable={args[-1]}/yebisu", file=sys.stderr)
        sys.stdout.buffer.write(plistlib.dumps({"application-identifier": env("MAC_TEST_SIGNED_APP_ID")}))
    ''',
    # Opening an .ipa installs Applications/yebisu.app (MAC_TEST_INSTALL=yes) or
    # a numbered wrapper (numbered), as macOS did on 2026-11-24 beside an
    # existing one; opening a wrapper starts it from a translocated copy.
    "open": r'''
        log("open")
        applications = pathlib.Path(env("YEBISU_MAC_APPLICATIONS"))
        if args[0].endswith(".ipa"):
            alive = []
            for entry in registry():
                try:
                    os.kill(entry["pid"], 0)
                    alive.append(entry["pid"])
                except ProcessLookupError:
                    pass
            (pathlib.Path(env("MAC_TEST_ROOT")) / "alive-at-install.json").write_text(json.dumps(alive))
            mode = env("MAC_TEST_INSTALL", "yes")
            if mode == "fail":
                sys.exit(1)
            target = applications / ("yebisu 2.app" if mode == "numbered" else "yebisu.app")
            if mode in ("yes", "numbered"):
                scratch = tempfile.mkdtemp()
                subprocess.run(["ditto", "-x", "-k", args[0], scratch], check=True)
                shutil.rmtree(target / "Wrapper" / "yebisu.app", ignore_errors=True)
                shutil.copytree(pathlib.Path(scratch) / "Payload" / "yebisu.app", target / "Wrapper" / "yebisu.app")
        elif args[0].endswith(".app"):
            mode = env("MAC_TEST_LAUNCH", "yes")
            if mode == "fail":
                sys.exit(1)
            if mode == "yes":
                pid = spawn()
                copy = pathlib.Path(env("MAC_TEST_ROOT")) / "X" / str(pid) / "d" / "Wrapper" / "yebisu.app"
                shutil.copytree(pathlib.Path(args[0]) / "Wrapper" / "yebisu.app", copy)
                listings = env("MAC_TEST_LISTINGS")
                registry(registry() + [{"pid": pid, "ppid": int(env("MAC_TEST_PARENT", "1")),
                                        "exe": str(copy / "yebisu"),
                                        "listings": int(listings) if listings else None}])
                container = pathlib.Path(env("HOME")) / "Library/Containers/4A4312EF-TEST"
                logfile = container / "Data" / env("MAC_TEST_LOG_SOURCE")
                logfile.parent.mkdir(parents=True, exist_ok=True)
                (container / ".com.apple.containermanagerd.metadata.plist").write_bytes(
                    plistlib.dumps({"MCMMetadataIdentifier": "com.shin1ohno.yebisu"}))
                with open(logfile, "a") as out:
                    out.write(json.dumps({"event": "startup", "pid": pid}, separators=(",", ":")) + "\n")
        else:
            sys.exit(64)
    ''',
    "ps": r'''
        log("ps")
        entries = registry()
        for entry in entries:
            try:
                os.kill(entry["pid"], 0)
            except ProcessLookupError:
                continue
            if entry.get("listings") is not None:
                if entry["listings"] <= 0:
                    continue
                entry["listings"] -= 1
            print(f'{entry["pid"]:>5} {entry["ppid"]:>5} {entry["exe"]}')
        registry(entries)
    ''',
}
STUB = textwrap.dedent('''\
    python3 -I -c 'import json, os, sys; open(os.environ["MAC_TEST_LOG"], "a").write(json.dumps(
        {{"tool": sys.argv[1], "args": sys.argv[2:], "rustup": os.environ.get("RUSTUP_TOOLCHAIN")}}) + "\\n")' \\
      {name} "$@"
    if [[ -n ${{MAC_TEST_PROVISION_STDERR:-}} && {name} == provision.sh ]]; then
      printf '%s\\n' "$MAC_TEST_PROVISION_STDERR" >&2
    fi
    echo "diagnostics on stdout before the result"
    printf '%s\\n' "${result}"
    exit "${{{status}:-0}}"
''')


def profile_plist(devices=(MAC, OTHER_DEVICE), all_devices=False):
    document = {
        "Name": "iOS Team Provisioning Profile: com.shin1ohno.yebisu",
        "TeamIdentifier": [TEAM], "Platform": ["iOS", "xrOS", "visionOS"],
        "ExpirationDate": datetime.now(timezone.utc).replace(tzinfo=None) + timedelta(days=30),
        "ProvisionedDevices": list(devices),
        "Entitlements": {"application-identifier": f"{TEAM}.{BUNDLE}"},
    }
    if all_devices:
        document["ProvisionsAllDevices"] = True
    return plistlib.dumps(document)


def alive(pid):
    command = subprocess.run(["/bin/ps", "-o", "command=", "-p", str(pid)],
                             capture_output=True, text=True, check=False).stdout.strip()
    return command == "sleep 600"


class Harness:
    """A copied tree whose provision.sh and bundle.sh are stubs, with fake tools."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="yebisu-mac-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.addCleanup(self.stop_sleepers)
        tools = self.root / "tools"
        tools.mkdir()
        for name, body in FAKES.items():
            (tools / name).write_text("#!/usr/bin/env python3\n" + PRELUDE + textwrap.dedent(body))
            (tools / name).chmod(0o755)
        self.home = self.root / "home"
        (self.home / "Library/Containers").mkdir(parents=True)
        self.applications = self.root / "Applications"
        self.applications.mkdir()
        self.registry = self.root / "processes.json"
        self.registry.write_text("[]")
        self.log = self.root / "calls.jsonl"
        self.out = self.root / "out"
        self.profile = self.write_profile(self.root / "Provisioning Profiles" / "8563c434 test.mobileprovision")
        self.app = self.make_app(self.root / "build with spaces" / "yebisu.app")
        self.script = self.scripted_tree(self.root / "tree")
        environment = {k: v for k, v in os.environ.items()
                       if not k.startswith(("HERDR_IOS_", "YEBISU_", "RUSTUP_TOOLCHAIN"))}
        self.env = {
            **environment, "HOME": str(self.home), "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "MAC_TEST_LOG": str(self.log), "MAC_TEST_ROOT": str(self.root),
            "MAC_TEST_PROCESSES": str(self.registry), "MAC_TEST_LOG_SOURCE": LOG_SOURCE,
            "MAC_TEST_HARDWARE": HARDWARE, "MAC_TEST_VALID": VALID, "MAC_TEST_LISTED": UNTRUSTED,
            "MAC_TEST_SIGNED_APP_ID": f"{TEAM}.{BUNDLE}", "MAC_TEST_PROFILE": str(self.profile),
            "MAC_TEST_APP": str(self.app), "YEBISU_MAC_APPLICATIONS": str(self.applications),
            "YEBISU_MAC_TIMEOUT": "2",
        }

    def scripted_tree(self, tree):
        scripts = tree / "scripts" / "ios"
        scripts.mkdir(parents=True)
        for name in ("mac.sh", "mac.py", "profile.py"):
            shutil.copy(HERE / name, scripts / name)
        location = tree / "crates/yebisu/src/log-location.txt"
        location.parent.mkdir(parents=True)
        shutil.copy(REPOSITORY / "crates/yebisu/src/log-location.txt", location)
        for name, result, status in (("provision.sh", "MAC_TEST_PROFILE", "MAC_TEST_PROVISION_STATUS"),
                                     ("bundle.sh", "MAC_TEST_APP", "MAC_TEST_BUNDLE_STATUS")):
            (scripts / name).write_text(STUB.format(name=name, result="{" + result + "}", status=status))
        return scripts / "mac.sh"

    def write_profile(self, path, **kwargs):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(profile_plist(**kwargs))
        return path

    def make_app(self, path, bundle_id=BUNDLE, devices=(MAC,)):
        path.mkdir(parents=True)
        (path / "Info.plist").write_bytes(plistlib.dumps(
            {"CFBundleIdentifier": bundle_id, "CFBundleExecutable": "yebisu"}))
        # Each build's signature carries its signing time, so executables differ.
        (path / "yebisu").write_bytes(os.urandom(64))
        self.write_profile(path / "embedded.mobileprovision", devices=devices)
        return path

    def running(self, parts, bundle_id=BUNDLE, executable="yebisu", ignore_term=False):
        """Start a sleep that ps reports as running ROOT/PARTS/EXECUTABLE."""
        trap = "trap '' TERM; " if ignore_term else ""
        started = subprocess.run(["/bin/sh", "-c", f"({trap}exec sleep 600) </dev/null >/dev/null 2>&1 & echo $!"],
                                 capture_output=True, text=True, check=True)
        pid = int(started.stdout)
        bundle = self.root.joinpath(*parts)
        bundle.mkdir(parents=True)
        (bundle / "Info.plist").write_bytes(plistlib.dumps(
            {"CFBundleIdentifier": bundle_id, "CFBundleExecutable": executable}))
        entries = json.loads(self.registry.read_text())
        entries.append({"pid": pid, "ppid": 1, "exe": str(bundle / executable), "listings": None})
        self.registry.write_text(json.dumps(entries))
        return pid

    def stop_sleepers(self):
        for entry in json.loads(self.registry.read_text()):
            if alive(entry["pid"]):
                os.kill(entry["pid"], signal.SIGKILL)

    def run_mac(self, *args, script=None):
        return subprocess.run(
            ["bash", str(script or self.script), "--team", TEAM, "--wait", "0", "--out", str(self.out), *args],
            env=self.env, text=True, capture_output=True, check=False)

    def calls(self, tool=None):
        if not self.log.exists():
            return []
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        return [call for call in calls if tool is None or call["tool"] == tool]

    def opened(self, kind):
        return [c for c in self.calls("open") if c["args"][0].endswith(f".{kind}")]

    def wrapper(self, name, bundle_id=BUNDLE):
        """An installed wrapper from an earlier install, with another build's executable."""
        bundle = self.applications / name / "Wrapper" / "yebisu.app"
        bundle.mkdir(parents=True)
        (bundle / "Info.plist").write_bytes(plistlib.dumps(
            {"CFBundleIdentifier": bundle_id, "CFBundleExecutable": "yebisu"}))
        (bundle / "yebisu").write_bytes(b"an earlier build")
        return self.applications / name

    def assert_refused(self, result, step, status):
        self.assertEqual(result.returncode, status, result.stdout + result.stderr)
        self.assertTrue(result.stderr.splitlines()[-1].startswith(f"error: just mac failed at step {step}:"),
                        result.stderr)


@unittest.skipUnless(sys.platform == "darwin", "mac.sh reads bundles with plutil and zips with ditto")
class MacScriptTests(Harness, unittest.TestCase):
    def test_happy_path_runs_steps_in_order(self):
        result = self.run_mac()
        self.assertEqual(result.returncode, 0, result.stderr)
        labels = []
        for call in self.calls():
            tool, args = call["tool"], call["args"]
            label = {"system_profiler": "hardware", "provision.sh": "provision", "bundle.sh": "build"}.get(tool)
            if tool == "security":
                label = "identity" if args[0] == "find-identity" else "profile check"
            elif tool == "codesign" and "--verify" in args:
                label = "verify"
            elif tool == "open":
                label = "install" if args[0].endswith(".ipa") else "launch"
            elif tool == "ps" and "launch" in labels:
                label = "running check"
            if label and label not in labels:
                labels.append(label)
        self.assertEqual(labels, ["hardware", "identity", "provision", "profile check", "build",
                                  "verify", "install", "launch", "running check"])
        (entry,) = json.loads(self.registry.read_text())
        installed = self.applications / "yebisu.app/Wrapper/yebisu.app"
        self.assertEqual((installed / "yebisu").read_bytes(), (self.app / "yebisu").read_bytes())
        self.assertEqual(result.stdout.splitlines(), [
            f"app:        {installed}", f"bundle id:  {BUNDLE}", f"device:     this Mac ({MAC})",
            f"state:      running (pid {entry['pid']})", f"log:        {self.out / 'yebisu.jsonl'}"])
        self.assertEqual((self.out / "summary.txt").read_text(), result.stdout)
        self.assertIn(f"parent pid 1\nexecutable {entry['exe']}\n", (self.out / "launch.txt").read_text())
        self.assertIn(f'"pid":{entry["pid"]}}}', (self.out / "yebisu.jsonl").read_text())
        self.assertTrue(alive(entry["pid"]))

    def test_provision_gets_this_macs_udid_and_designed_for_ipad_destination(self):
        result = self.run_mac()
        self.assertEqual(result.returncode, 0, result.stderr)
        (provision,) = self.calls("provision.sh")
        args = provision["args"]
        self.assertEqual(args[args.index("--device") + 1], MAC)
        self.assertEqual(args[args.index("--destination") + 1], DESTINATION)
        self.assertEqual(args[args.index("--bundle-id") + 1], BUNDLE)
        self.assertNotIn("--register-device", args)
        (bundle,) = self.calls("bundle.sh")
        self.assertIn("--release", bundle["args"])
        self.assertEqual(bundle["args"][bundle["args"].index("--target") + 1], "aarch64-apple-ios")

    def test_dev_option_builds_with_the_dev_profile(self):
        result = self.run_mac("--dev")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("--release", self.calls("bundle.sh")[0]["args"])

    def test_profile_path_with_a_space_reaches_signing_intact(self):
        self.assertIn(" ", str(self.profile))
        result = self.run_mac()
        self.assertEqual(result.returncode, 0, result.stderr)
        args = self.calls("bundle.sh")[0]["args"]
        self.assertEqual(args[args.index("--profile") + 1], str(self.profile))
        self.assertIn(["cms", "-D", "-i", str(self.profile)], [c["args"] for c in self.calls("security")])

    def test_clears_rustup_toolchain_so_rust_toolchain_toml_applies(self):
        self.env["RUSTUP_TOOLCHAIN"] = "1.100.0"
        result = self.run_mac()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Clearing RUSTUP_TOOLCHAIN=1.100.0 so rust-toolchain.toml applies", result.stderr)
        for stub in self.calls("provision.sh") + self.calls("bundle.sh"):
            self.assertIsNone(stub["rustup"], stub)

    def test_register_recipe_passes_register_device_and_stops(self):
        result = self.run_mac("--register-device", "--register-only")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--register-device", self.calls("provision.sh")[0]["args"])
        self.assertEqual(result.stdout.splitlines()[-1], f"profile: {self.profile}")
        self.assertFalse(self.calls("bundle.sh"))
        self.assertFalse(self.calls("open"))

    def test_refuses_other_bundle_id_before_any_tool(self):
        self.assert_refused(self.run_mac("--bundle-id", "com.example.other"), "options", 2)
        self.assertEqual(self.calls(), [])

    def test_refuses_profile_not_listing_this_mac(self):
        self.env["MAC_TEST_PROFILE"] = str(self.write_profile(self.root / "p.mobileprovision", devices=[OTHER_DEVICE]))
        self.assert_refused(self.run_mac(), "profile-check", 1)
        self.assertFalse(self.calls("bundle.sh"))

    def test_refuses_profile_that_only_provisions_all_devices(self):
        self.env["MAC_TEST_PROFILE"] = str(self.write_profile(
            self.root / "p.mobileprovision", devices=[], all_devices=True))
        self.assert_refused(self.run_mac(), "profile-check", 1)
        self.assertFalse(self.calls("bundle.sh"))

    def test_refuses_built_bundle_with_other_identifier(self):
        self.env["MAC_TEST_APP"] = str(self.make_app(self.root / "other" / "yebisu.app", bundle_id="com.example.other"))
        result = self.run_mac()
        self.assert_refused(result, "bundle-check", 2)
        self.assertIn("'com.example.other'", result.stderr)
        self.assertFalse(self.calls("open"))

    def test_refuses_signature_for_other_app_id(self):
        self.env["MAC_TEST_SIGNED_APP_ID"] = f"{TEAM}.com.example.other"
        self.assert_refused(self.run_mac(), "bundle-check", 2)
        self.assertFalse(self.calls("open"))

    def test_refuses_bundle_whose_embedded_profile_omits_this_mac(self):
        app = self.make_app(self.root / "given" / "yebisu.app", devices=[OTHER_DEVICE])
        self.assert_refused(self.run_mac("--app", str(app)), "bundle-check", 1)
        self.assertFalse(self.calls("provision.sh"))
        self.assertFalse(self.calls("open"))

    def test_unregistered_mac_names_the_owner_step(self):
        self.env.update(MAC_TEST_PROVISION_STATUS="1", MAC_TEST_PROVISION_STDERR=UNREGISTERED)
        result = self.run_mac()
        self.assert_refused(result, "register", 3)
        self.assertIn("'just mac-register' once", result.stderr.splitlines()[-1])
        self.assertIn(UNREGISTERED, result.stderr)
        self.assertNotIn("--register-device", self.calls("provision.sh")[0]["args"])
        self.assertFalse(self.calls("bundle.sh"))

    def test_other_provision_failure_is_step_provision(self):
        self.env.update(MAC_TEST_PROVISION_STATUS="1", MAC_TEST_PROVISION_STDERR="error: No Account for Team")
        self.assert_refused(self.run_mac(), "provision", 1)
        self.assertFalse(self.calls("bundle.sh"))

    def test_untrusted_identity_names_the_wwdr_intermediate(self):
        self.env["MAC_TEST_VALID"] = NONE_VALID
        result = self.run_mac()
        self.assert_refused(result, "preflight", 1)
        self.assertIn("WWDR CA G3", result.stderr)
        self.assertFalse(self.calls("provision.sh"))

    def test_no_identity_at_all(self):
        self.env.update(MAC_TEST_VALID=NONE_VALID, MAC_TEST_LISTED=NO_IDENTITY)
        result = self.run_mac()
        self.assert_refused(result, "preflight", 1)
        self.assertIn("no Apple Development signing identity", result.stderr)

    def test_missing_provisioning_udid_fails_preflight(self):
        self.env["MAC_TEST_HARDWARE"] = HARDWARE.replace(f"Provisioning UDID: {MAC}", "")
        result = self.run_mac()
        self.assert_refused(result, "preflight", 1)
        self.assertIn("Provisioning UDID", result.stderr)
        self.env.update(MAC_TEST_HARDWARE=HARDWARE, MAC_TEST_ARCH="x86_64")
        self.assert_refused(self.run_mac(), "preflight", 1)
        self.assertFalse(self.calls("provision.sh"))

    def test_build_failure_is_step_build(self):
        self.env["MAC_TEST_BUNDLE_STATUS"] = "1"
        self.assert_refused(self.run_mac(), "build", 1)
        self.assertFalse(self.calls("open"))

    def test_exited_app_is_status_4_after_summary(self):
        self.env["MAC_TEST_LISTINGS"] = "1"
        result = self.run_mac()
        self.assert_refused(result, "running", 4)
        self.assertIn("state:      exited", result.stdout)
        self.assertEqual(result.stderr.splitlines()[-1],
                         "error: just mac failed at step running: the app exited within about 0s")

    def test_install_failure_and_launch_failure_name_their_steps(self):
        for variable, mode, step in (("MAC_TEST_INSTALL", "fail", "install"), ("MAC_TEST_INSTALL", "never", "install"),
                                     ("MAC_TEST_LAUNCH", "fail", "launch"), ("MAC_TEST_LAUNCH", "none", "launch")):
            with self.subTest(variable=variable, mode=mode):
                self.env.pop("MAC_TEST_INSTALL", None)
                self.env.pop("MAC_TEST_LAUNCH", None)
                shutil.rmtree(self.applications / "yebisu.app", ignore_errors=True)  # the scratch install
                self.env[variable] = mode
                result = self.run_mac()
                self.assert_refused(result, step, 1)
                if step == "launch":
                    self.assertIn("state:      launch failed", result.stdout)

    def test_never_touches_another_apps_bundle(self):
        for info in ("yebisu.app/Contents/Info.plist", "yebisu.app/Wrapper/yebisu.app/Info.plist"):
            with self.subTest(info=info):
                shutil.rmtree(self.applications / "yebisu.app", ignore_errors=True)
                self.log.unlink(missing_ok=True)
                other = self.applications / info
                other.parent.mkdir(parents=True)
                other.write_bytes(plistlib.dumps({"CFBundleIdentifier": "com.example.yebisu"}))
                result = self.run_mac()
                self.assert_refused(result, "install", 1)
                self.assertIn("is another app (com.example.yebisu)", result.stderr)
                self.assertEqual(plistlib.loads(other.read_bytes()), {"CFBundleIdentifier": "com.example.yebisu"})
                self.assertFalse(self.calls("open"))

    def test_terminates_only_the_installed_bundles_earlier_process(self):
        earlier = self.running(("X", "OLD", "d", "Wrapper", "yebisu.app"))
        host_build = self.running(("target", "debug"))
        other_bundle = self.running(("X", "OTHER", "d", "Wrapper", "yebisu.app"), bundle_id="com.example.other")
        unwrapped = self.running(("target", "aarch64-apple-ios", "release", "yebisu.app"))
        result = self.run_mac()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"Terminating the earlier {BUNDLE} (pid {earlier})", result.stderr)
        self.assertFalse(alive(earlier))
        self.assertNotIn(earlier, json.loads((self.root / "alive-at-install.json").read_text()))
        for survivor in (host_build, other_bundle, unwrapped):
            self.assertTrue(alive(survivor), survivor)
        self.assertEqual(result.stderr.count("Terminating"), 1)
        launched = json.loads(self.registry.read_text())[-1]["pid"]
        self.assertIn(f"state:      running (pid {launched})", result.stdout)

    def test_earlier_copy_that_will_not_stop_fails_before_install(self):
        stubborn = self.running(("X", "OLD", "d", "Wrapper", "yebisu.app"), ignore_term=True)
        result = self.run_mac()
        self.assert_refused(result, "terminate", 1)
        self.assertIn(f"did not stop: {stubborn} ", result.stderr)
        self.assertFalse(self.opened("ipa"))

    def test_existing_wrappers_stop_the_install_and_are_named(self):
        named = self.wrapper("yebisu.app")
        running = self.running(("Applications", "yebisu 2.app", "Wrapper", "yebisu.app"))
        numbered = self.applications / "yebisu 2.app"
        result = self.run_mac()
        self.assert_refused(result, "install", 1)
        self.assertEqual(result.stderr.splitlines()[-1],
                         f"error: just mac failed at step install: {BUNDLE} is already installed as "
                         f"{numbered}, {named}; move each to the Trash in Finder (it asks for an "
                         "administrator password), then run 'just mac' again")
        self.assertTrue(alive(running), "a refused run must not stop the running copy")
        self.assertNotIn("Terminating", result.stderr)
        self.assertEqual((named / "Wrapper/yebisu.app/yebisu").read_bytes(), b"an earlier build")
        self.assertTrue((numbered / "Wrapper/yebisu.app/Info.plist").exists())
        self.assertFalse(self.calls("open"))

    def test_new_numbered_wrapper_is_refused_and_nothing_is_removed(self):
        self.env["MAC_TEST_INSTALL"] = "numbered"
        result = self.run_mac()
        self.assert_refused(result, "install", 1)
        self.assertIn(f"as {self.applications / 'yebisu 2.app'}, not {self.applications / 'yebisu.app'};"
                      " numbered wrappers are not used", result.stderr)
        self.assertTrue((self.applications / "yebisu 2.app/Wrapper/yebisu.app/yebisu").exists())
        self.assertFalse(self.opened("app"))

    def test_app_under_another_parent_fails_the_running_check(self):
        self.env["MAC_TEST_PARENT"] = "4242"
        result = self.run_mac()
        self.assert_refused(result, "running", 1)
        self.assertIn("has parent pid 4242, not launchd (1)", result.stderr.splitlines()[-1])

    def test_no_launch_installs_only(self):
        result = self.run_mac("--no-launch")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.opened("ipa"))
        self.assertFalse(self.opened("app"))
        self.assertIn("state:      installed, not launched", result.stdout)


class RecipeTests(unittest.TestCase):
    def recipe(self, header):
        lines = (REPOSITORY / "justfile").read_text().splitlines()
        return lines[lines.index(header) + 1].strip()

    def test_mac_recipe_runs_mac_sh_through_the_guard(self):
        call = "bash scripts/ios/mac.sh --bundle-id com.shin1ohno.yebisu --team AL9567565U"
        mac_body, register_body = self.recipe('mac wait="5":'), self.recipe("mac-register:")
        for body in (mac_body, register_body):
            self.assertTrue(body.startswith("python3 scripts/guarded.py "), body)
            self.assertIn(call, body)
        self.assertNotIn("--register-device", mac_body)
        self.assertTrue(register_body.endswith("--register-device --register-only"), register_body)

    def test_ipad_recipe_is_unchanged(self):
        self.assertEqual(self.recipe("ipad:"), "python3 scripts/guarded.py env CARGO_INCREMENTAL=1 bash "
                         "scripts/ios/device.sh --device M5 --bundle-id com.shin1ohno.yebisu --team AL9567565U")


class MacHelperTests(unittest.TestCase):
    def test_udid_reads_the_hardware_overview_and_rejects_others(self):
        self.assertEqual(mac.provisioning_udid(HARDWARE), MAC)
        self.assertIsNone(mac.provisioning_udid(HARDWARE.replace(MAC, "")))
        self.assertIsNone(mac.provisioning_udid(HARDWARE.replace(MAC, "00006050-0004")))
        self.assertIsNone(mac.provisioning_udid("Serial Number (system): DGYK09TW31\n"))

    def test_identities_counts_apple_development_lines(self):
        self.assertEqual(len(mac.IDENTITY.findall(VALID)), 1)
        self.assertEqual(len(mac.IDENTITY.findall(NONE_VALID)), 0)
        self.assertEqual(len(mac.IDENTITY.findall(UNTRUSTED)), 1)

    def test_profile_lists_only_an_exact_provisioned_device(self):
        listed = plistlib.loads(profile_plist())
        self.assertIsNone(mac.lists_device(listed, MAC))
        self.assertIn("does not list", mac.lists_device(listed, "DGYK09TW31"))
        everyone = plistlib.loads(profile_plist(devices=[], all_devices=True))
        self.assertIn("does not list", mac.lists_device(everyone, MAC))
        self.assertIn("malformed", mac.lists_device({**listed, "ProvisionedDevices": MAC}, MAC))

    def test_entitlement_app_id_reads_codesign_xml(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "entitlements.xml"
            path.write_bytes(plistlib.dumps({"application-identifier": f"{TEAM}.{BUNDLE}",
                                             "get-task-allow": True}))
            printed = io.StringIO()
            with redirect_stdout(printed):
                self.assertEqual(mac.main(["entitlement-app-id", str(path)]), 0)
            self.assertEqual(printed.getvalue(), f"{TEAM}.{BUNDLE}\n")
            path.write_bytes(plistlib.dumps({"get-task-allow": True}))
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                self.assertEqual(mac.main(["entitlement-app-id", str(path)]), 1)

    def test_processes_and_container_match_the_bundle_identifier(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            for name, bundle_id in (("mine", BUNDLE), ("other", "com.example.other")):
                bundle = root / name / "Wrapper" / "yebisu.app"
                bundle.mkdir(parents=True)
                (bundle / "Info.plist").write_bytes(plistlib.dumps(
                    {"CFBundleIdentifier": bundle_id, "CFBundleExecutable": "yebisu"}))
                container = root / "Containers" / name
                container.mkdir(parents=True)
                (container / mac.METADATA).write_bytes(plistlib.dumps({"MCMMetadataIdentifier": bundle_id}))
            listing = (f"  101     1 {root}/mine/Wrapper/yebisu.app/yebisu\n"
                       f"  102     1 {root}/other/Wrapper/yebisu.app/yebisu\n"
                       f"  103    50 {root}/target/debug/yebisu\n  bad line\n")
            self.assertEqual(mac.processes(listing, BUNDLE),
                             [(101, 1, f"{root}/mine/Wrapper/yebisu.app/yebisu")])
            self.assertEqual(mac.processes(listing, BUNDLE, pid=102), [])
            self.assertEqual(mac.container(root / "Containers", BUNDLE), root / "Containers" / "mine")
            self.assertIsNone(mac.container(root / "Containers", "com.example.absent"))


if __name__ == "__main__":
    unittest.main()
