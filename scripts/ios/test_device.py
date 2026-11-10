"""Tests for scripts/ios/device.sh and devicectl.py against a fake xcrun.

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_device.py'

No device, Xcode, or build is involved: xcrun is a script that answers
devicectl's commands from fixtures shaped like Xcode 26's JSON output.
"""

import importlib.util
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

SCRIPT = Path(__file__).with_name("device.sh")
SPEC = importlib.util.spec_from_file_location("devicectl", Path(__file__).with_name("devicectl.py"))
devicectl = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(devicectl)

BUNDLE = "com.shin1ohno.yebisu"
LOG_SOURCE = (SCRIPT.parents[2] / "crates/yebisu/src/log-location.txt").read_text().strip()


def device(name, identifier, udid, device_type="iPad", tunnel="connected",
           reality="physical", platform="iOS", pairing="paired", developer_mode="enabled"):
    return {
        "identifier": identifier,
        "connectionProperties": {"pairingState": pairing, "tunnelState": tunnel},
        "deviceProperties": {"name": name, "developerModeStatus": developer_mode},
        "hardwareProperties": {
            "deviceType": device_type, "platform": platform, "reality": reality, "udid": udid,
        },
    }


IPAD = device("Studio iPad", "AAAAAAAA-0000-0000-0000-000000000001", "00000000-000000000000000A")
OTHER_IPAD = device("Travel iPad", "BBBBBBBB-0000-0000-0000-000000000002", "00000000-000000000000000B")
IPHONE = device("Phone", "CCCCCCCC-0000-0000-0000-000000000003", "00000000-000000000000000C",
                device_type="iPhone")
MAC = device("Mac", "DDDDDDDD-0000-0000-0000-000000000004", "00000000-000000000000000D",
             device_type="Mac", platform="macOS")
SIMULATOR = device("iPad Simulator", "EEEEEEEE-0000-0000-0000-000000000005", "sim",
                   reality="virtual")


def listing(*devices):
    return {"info": {"outcome": "success"}, "result": {"devices": list(devices)}}


class ResolveTests(unittest.TestCase):
    def test_the_only_physical_ipad_is_the_default(self):
        chosen = devicectl.resolve(listing(IPHONE, MAC, SIMULATOR, IPAD))
        self.assertEqual(chosen["identifier"], IPAD["identifier"])
        self.assertEqual(chosen["udid"], "00000000-000000000000000A")

    def test_wanted_matches_name_identifier_or_udid(self):
        document = listing(IPAD, OTHER_IPAD)
        for wanted in ("Travel iPad", OTHER_IPAD["identifier"], "00000000-000000000000000B"):
            self.assertEqual(devicectl.resolve(document, wanted)["name"], "Travel iPad")

    def test_several_ipads_need_a_choice(self):
        with self.assertRaisesRegex(devicectl.Unresolved, "several devices"):
            devicectl.resolve(listing(IPAD, OTHER_IPAD))

    def test_simulators_never_match(self):
        with self.assertRaises(devicectl.Unresolved):
            devicectl.resolve(listing(SIMULATOR), "iPad Simulator")

    def test_unpaired_or_without_developer_mode_is_not_ready(self):
        for broken in (
            device("x", "X", "U", pairing="unpaired"),
            device("x", "X", "U", developer_mode="disabled"),
        ):
            with self.assertRaises(devicectl.Unresolved) as raised:
                devicectl.resolve(listing(broken))
            self.assertEqual(raised.exception.status, 3)

    def test_device_without_a_hardware_udid_is_not_ready(self):
        nameless = device("Studio iPad", IPAD["identifier"], "", tunnel="")
        del nameless["hardwareProperties"]["udid"]
        with self.assertRaisesRegex(devicectl.Unresolved, "no hardware UDID") as raised:
            devicectl.resolve(listing(nameless))
        self.assertEqual(raised.exception.status, 3)

    def test_process_readers_find_identifiers_at_any_depth(self):
        launch = {"result": {"process": {"processIdentifier": 4242}}}
        self.assertEqual(devicectl.launched_pid(launch), 4242)
        processes = {"result": {"runningProcesses": [{"processIdentifier": 1},
                                                     {"processIdentifier": 4242}]}}
        self.assertTrue(devicectl.running(processes, 4242))
        self.assertFalse(devicectl.running(processes, 7))
        self.assertEqual(devicectl.error_code({"error": {"code": 3002}}), 3002)
        self.assertIsNone(devicectl.error_code({"result": {}}))


@unittest.skipUnless(sys.platform == "darwin", "device.sh reads the bundle with plutil")
class DeviceScriptTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="herdr-ios-device-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        tools = self.root / "tools"
        tools.mkdir()
        self.log = self.root / "calls.jsonl"
        self.devices = self.root / "devices.json"
        self.devices.write_text(json.dumps(listing(IPAD, IPHONE, SIMULATOR)))
        self.app = self.root / "build with spaces" / "herdr-gpui.app"
        self.app.mkdir(parents=True)
        (self.app / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": BUNDLE}))
        (self.app / "embedded.mobileprovision").write_bytes(b"isolated profile")
        self.out = self.root / "out"
        environment = {
            key: value for key, value in os.environ.items() if not key.startswith("HERDR_IOS_")
        }
        self.env = {
            **environment,
            "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "HERDR_DEVICE_TEST_LOG": str(self.log),
            "HERDR_DEVICE_TEST_DEVICES": str(self.devices),
            "HERDR_DEVICE_TEST_STATE": str(self.root / "state"),
        }
        script = tools / "xcrun"
        script.write_text("#!/usr/bin/env python3\n" + textwrap.dedent(r'''
            import json, os, pathlib, sys
            args = sys.argv[1:]
            with open(os.environ["HERDR_DEVICE_TEST_LOG"], "a") as log:
                log.write(json.dumps(args) + "\n")
            state = pathlib.Path(os.environ["HERDR_DEVICE_TEST_STATE"])
            def option(name):
                return args[args.index(name) + 1]
            def write_json(document):
                pathlib.Path(option("--json-output")).write_text(json.dumps(document))
            command = args[1:4]
            if args[1:3] == ["list", "devices"]:
                if os.environ.get("HERDR_DEVICE_TEST_FAILS") == "devices":
                    print("ERROR: CoreDevice is not running", file=sys.stderr)
                    sys.exit(1)
                pathlib.Path(option("--json-output")).write_text(
                    pathlib.Path(os.environ["HERDR_DEVICE_TEST_DEVICES"]).read_text())
            elif command == ["device", "install", "app"]:
                failures = os.environ.get("HERDR_DEVICE_TEST_INSTALL_FAILS", "")
                attempts = state.with_suffix(".installs")
                count = int(attempts.read_text()) if attempts.exists() else 0
                attempts.write_text(str(count + 1))
                if count < len(failures.split()):
                    code = int(failures.split()[count])
                    write_json({"error": {"code": code, "domain": "com.apple.dt.CoreDeviceError"}})
                    print(f"ERROR: failed (com.apple.dt.CoreDeviceError error {code}.)", file=sys.stderr)
                    sys.exit(1)
                write_json({"result": {"installedApplications": [{"bundleID": "x"}]}})
            elif command[1:] in (["process", "launch"], ["info", "processes"]) and \
                    os.environ.get("HERDR_DEVICE_TEST_FAILS") == command[2]:
                print(f"ERROR: {command[2]} failed (CoreDeviceError 4000)", file=sys.stderr)
                sys.exit(1)
            elif command == ["device", "process", "launch"]:
                write_json({"result": {"process": {"processIdentifier": 4242}}})
            elif command == ["device", "info", "processes"]:
                running = [{"processIdentifier": 1}]
                if os.environ.get("HERDR_DEVICE_TEST_EXITS") is None:
                    running.append({"processIdentifier": 4242})
                write_json({"result": {"runningProcesses": running}})
            elif command == ["device", "copy", "from"]:
                attempts = state.with_suffix(".copies")
                count = int(attempts.read_text()) if attempts.exists() else 0
                attempts.write_text(str(count + 1))
                missing = int(os.environ.get("HERDR_DEVICE_TEST_MISSING", "0"))
                failure = os.environ.get("HERDR_DEVICE_TEST_COPY_ERROR")
                if count < missing or failure:
                    code = int(failure or "7000")
                    pathlib.Path(option("--destination")).write_text("partial transfer")
                    if not os.environ.get("HERDR_DEVICE_TEST_NO_JSON"):
                        write_json({"error": {"code": code}})
                    print(f"ERROR: CoreDeviceError error {code}", file=sys.stderr)
                    sys.exit(1)
                empty = int(os.environ.get("HERDR_DEVICE_TEST_EMPTY", "0"))
                pathlib.Path(option("--destination")).write_text(
                    "" if count < empty else '{"level":"INFO"}\n')
            else:
                sys.exit(64)
        ''').lstrip())
        script.chmod(0o755)

    def run_device(self, *args, script=SCRIPT):
        return subprocess.run(
            ["bash", str(script), *args],
            env=self.env, text=True, capture_output=True, check=False,
        )

    def run_app(self, *args):
        return self.run_device("--app", str(self.app), "--wait", "0", "--out", str(self.out), *args)

    def calls(self, *prefix):
        if not self.log.exists():
            return []
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        return [call for call in calls if call[1:1 + len(prefix)] == list(prefix)]

    def test_installs_launches_and_copies_the_log_from_the_only_ipad(self):
        result = self.run_app()
        self.assertEqual(result.returncode, 0, result.stderr)
        install = self.calls("device", "install", "app")
        self.assertEqual(len(install), 1)
        self.assertEqual(install[0][install[0].index("--device") + 1], IPAD["identifier"])
        self.assertEqual(install[0][-1], str(self.app))
        launch = self.calls("device", "process", "launch")[0]
        self.assertEqual(launch[-1], BUNDLE)
        self.assertIn("--terminate-existing", launch)
        copy = self.calls("device", "copy", "from")[0]
        self.assertEqual(copy[copy.index("--timeout") + 1], "5")
        self.assertEqual(copy[copy.index("--domain-type") + 1], "appDataContainer")
        self.assertEqual(copy[copy.index("--domain-identifier") + 1], BUNDLE)
        self.assertEqual(
            copy[copy.index("--source") + 1],
            LOG_SOURCE,
        )
        self.assertTrue((self.out / "yebisu.jsonl").read_text())
        self.assertIn("running (pid 4242)", result.stdout)

    def test_missing_log_has_plain_message_and_launch_is_preserved(self):
        self.env["HERDR_DEVICE_TEST_MISSING"] = "99"
        result = self.run_app()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("log file does not exist yet", result.stdout)
        self.assertIn("running (pid 4242)", result.stdout)
        self.assertNotIn("CoreDeviceError", result.stdout + result.stderr)
        self.assertEqual(len(self.calls("device", "copy", "from")), 6)
        self.assertFalse((self.out / "yebisu.jsonl").exists())
        print("MISSING-FILE receiving summary:", result.stdout)

    def test_delayed_log_is_retried_and_copied(self):
        self.env["HERDR_DEVICE_TEST_MISSING"] = "2"
        result = self.run_app()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.calls("device", "copy", "from")), 3)
        self.assertTrue((self.out / "yebisu.jsonl").read_text())
        self.assertNotIn("does not exist yet", result.stdout)

    def test_missing_log_without_json_still_has_plain_message(self):
        self.env["HERDR_DEVICE_TEST_NO_JSON"] = "1"
        self.test_missing_log_has_plain_message_and_launch_is_preserved()

    def test_empty_copy_is_not_reported_as_collected(self):
        self.env["HERDR_DEVICE_TEST_EMPTY"] = "99"
        result = self.run_app()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("log file is still empty", result.stdout)
        self.assertIn("running (pid 4242)", result.stdout)
        self.assertEqual(len(self.calls("device", "copy", "from")), 6)
        self.assertFalse((self.out / "yebisu.jsonl").exists())

    def test_transfer_error_is_not_mislabeled_as_missing(self):
        self.env["HERDR_DEVICE_TEST_COPY_ERROR"] = "4000"
        result = self.run_app()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("transfer failed", result.stdout)
        self.assertIn("running (pid 4242)", result.stdout)
        self.assertEqual(len(self.calls("device", "copy", "from")), 1)

    def test_device_option_chooses_among_several_ipads(self):
        self.devices.write_text(json.dumps(listing(IPAD, OTHER_IPAD)))
        result = self.run_app()
        self.assertEqual(result.returncode, 1)
        self.assertIn("several devices", result.stderr)
        self.assertFalse(self.calls("device", "install", "app"))
        result = self.run_app("--device", "00000000-000000000000000B")
        self.assertEqual(result.returncode, 0, result.stderr)
        install = self.calls("device", "install", "app")[0]
        self.assertEqual(install[install.index("--device") + 1], OTHER_IPAD["identifier"])

    def test_unreachable_ipad_stops_before_installing(self):
        self.devices.write_text(json.dumps(listing(device(
            "Studio iPad", IPAD["identifier"], "U", tunnel="unavailable"))))
        result = self.run_app()
        self.assertEqual(result.returncode, 3)
        self.assertIn("unreachable", result.stderr)
        self.assertFalse(self.calls("device", "install", "app"))

    def test_developer_mode_is_required(self):
        self.devices.write_text(json.dumps(listing(device(
            "Studio iPad", IPAD["identifier"], "U", developer_mode="disabled"))))
        result = self.run_app()
        self.assertEqual(result.returncode, 3)
        self.assertIn("Developer Mode", result.stderr)

    def test_install_error_3002_is_retried_once(self):
        self.env["HERDR_DEVICE_TEST_INSTALL_FAILS"] = "3002"
        result = self.run_app()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.calls("device", "install", "app")), 2)

    def test_install_is_not_retried_twice_or_for_other_errors(self):
        self.env["HERDR_DEVICE_TEST_INSTALL_FAILS"] = "3002 3002"
        result = self.run_app()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(self.calls("device", "install", "app")), 2)
        self.log.unlink()
        (self.root / "state.installs").unlink()
        self.env["HERDR_DEVICE_TEST_INSTALL_FAILS"] = "3000"
        result = self.run_app()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(self.calls("device", "install", "app")), 1)
        self.assertIn("error 3000", result.stderr)

    def test_exited_app_fails_with_status_4_after_copying_its_log(self):
        self.env["HERDR_DEVICE_TEST_EXITS"] = "1"
        result = self.run_app()
        self.assertEqual(result.returncode, 4)
        self.assertIn("state:      exited", result.stdout)
        self.assertTrue((self.out / "yebisu.jsonl").exists())

    def test_no_launch_installs_only(self):
        result = self.run_app("--no-launch")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.calls("device", "install", "app"))
        self.assertFalse(self.calls("device", "process", "launch"))
        self.assertFalse(self.calls("device", "copy", "from"))

    def test_building_needs_a_team_before_any_device_command(self):
        result = self.run_device()
        self.assertEqual(result.returncode, 2)
        self.assertIn("--team", result.stderr)
        self.assertFalse(self.log.exists())

    def test_building_uses_constant_identity_with_no_environment_override(self):
        script = SCRIPT.read_text()
        self.assertIn("bundle_id=com.shin1ohno.yebisu", script)
        self.assertNotIn("bundle_id=${HERDR_IOS_BUNDLE_ID:-}", script)

    def test_device_without_a_udid_stops_before_installing(self):
        nameless = device("Studio iPad", IPAD["identifier"], "", tunnel="connected")
        del nameless["hardwareProperties"]["udid"]
        self.devices.write_text(json.dumps(listing(nameless)))
        result = self.run_app()
        self.assertEqual(result.returncode, 3)
        self.assertIn("no hardware UDID", result.stderr)
        self.assertFalse(self.calls("device", "install", "app"))

    def test_failed_device_listing_is_status_1(self):
        self.env["HERDR_DEVICE_TEST_FAILS"] = "devices"
        result = self.run_app()
        self.assertEqual(result.returncode, 1)
        self.assertIn("could not list devices", result.stderr)

    def test_failed_launch_or_listing_still_copies_the_log_and_reports(self):
        for failing, state in (("launch", "launch failed"),
                               ("processes", "unknown (processes not listed)")):
            self.env["HERDR_DEVICE_TEST_FAILS"] = failing
            result = self.run_app()
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn(f"state:      {state}", result.stdout)
            self.assertIn(f"log:        {self.out / 'yebisu.jsonl'}", result.stdout)
            self.assertTrue((self.out / "yebisu.jsonl").exists())

    def scripted_tree(self):
        """A copy of device.sh whose provision.sh and bundle.sh only log their arguments."""
        tree = self.root / "tree"
        scripts = tree / "scripts" / "ios"
        scripts.mkdir(parents=True)
        for name in ("device.sh", "devicectl.py"):
            shutil.copy(SCRIPT.with_name(name), scripts / name)
        location = tree / "crates/yebisu/src/log-location.txt"
        location.parent.mkdir(parents=True)
        shutil.copy(SCRIPT.parents[2] / "crates/yebisu/src/log-location.txt", location)
        self.script_log = self.root / "scripts.jsonl"
        self.env["HERDR_DEVICE_TEST_SCRIPTS"] = str(self.script_log)
        self.env["HERDR_DEVICE_TEST_APP"] = str(self.app)
        for name, output in (("provision.sh", "/isolated/profile.mobileprovision"),
                             ("bundle.sh", '"$HERDR_DEVICE_TEST_APP"')):
            (scripts / name).write_text(textwrap.dedent(f'''
                python3 -c 'import json, sys; print(json.dumps(sys.argv[1:]))' \
                  {name} "$@" >>"$HERDR_DEVICE_TEST_SCRIPTS"
                echo "diagnostics on stdout before the result"
                printf '%s\\n' {output}
            '''))
        return scripts / "device.sh"

    def script_calls(self, name):
        if not self.script_log.exists():
            return []
        calls = [json.loads(line) for line in self.script_log.read_text().splitlines()]
        return [call[1:] for call in calls if call[0] == name]

    def test_build_path_provisions_for_the_hardware_udid_then_signs_a_release(self):
        script = self.scripted_tree()
        for extra, release in (((), True), (("--dev",), False)):
            self.script_log.unlink(missing_ok=True)
            result = self.run_device(
                "--team", "TEAMID1234", "--bundle-id", BUNDLE, "--wait", "0",
                "--out", str(self.out), *extra, script=script,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            provision = self.script_calls("provision.sh")[0]
            self.assertEqual(provision[provision.index("--device") + 1], "00000000-000000000000000A")
            self.assertEqual(provision[provision.index("--bundle-id") + 1], BUNDLE)
            bundle = self.script_calls("bundle.sh")[0]
            self.assertEqual("--release" in bundle, release)
            self.assertIn("--no-default-features", bundle)
            self.assertEqual(bundle[bundle.index("--target") + 1], "aarch64-apple-ios")
            self.assertEqual(
                bundle[bundle.index("--profile") + 1], "/isolated/profile.mobileprovision"
            )
            install = self.calls("device", "install", "app")[-1]
            self.assertEqual(install[-1], str(self.app))

    def test_build_path_checks_the_ipad_before_provisioning(self):
        script = self.scripted_tree()
        self.devices.write_text(json.dumps(listing(device(
            "Studio iPad", IPAD["identifier"], "U", tunnel="unavailable"))))
        result = self.run_device("--team", "TEAMID1234", "--bundle-id", BUNDLE, script=script)
        self.assertEqual(result.returncode, 3)
        self.assertFalse(self.script_calls("provision.sh"))
        self.assertFalse(self.script_calls("bundle.sh"))

    def test_unsigned_bundle_and_bad_wait_are_refused(self):
        (self.app / "embedded.mobileprovision").unlink()
        self.assertEqual(self.run_app().returncode, 2)
        result = self.run_device("--app", str(self.app), "--wait", "soon")
        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
