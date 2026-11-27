"""Issue #256: CI's disposable Simulators keep the boot-task daemons out, and the job reports whether
they run anyway."""
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
from contextlib import redirect_stdout

REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("sim_ready", REPO / "scripts/ios/sim_ready.py")
sim_ready = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sim_ready)

RUNTIME = "/Library/Developer/CoreSimulator/Volumes/iOS_24B8117/Contents/Resources/RuntimeRoot"
PS = "\n".join([
    f"{RUNTIME}/System/Library/PrivateFrameworks/RemoteManagement.framework/remotemanagementd",
    f"{RUNTIME}/System/Library/PrivateFrameworks/RemoteManagement.framework/Subscribers/SecuritySubscriber",
    f"{RUNTIME}/System/Library/PrivateFrameworks/EmailDaemon.framework/maild",
    f"{RUNTIME}/usr/libexec/healthd",
    "/usr/libexec/remotemanagementd",  # the Mac's own daemon of the same name
    "/System/Library/PrivateFrameworks/EmailDaemon.framework/maild",
])


class Completed:
    def __init__(self, stdout="", returncode=0):
        self.stdout, self.stderr, self.returncode = stdout, "", returncode


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


class BootTaskProcesses(unittest.TestCase):
    def test_only_simulator_boot_task_daemons_and_subscribers_are_named(self):
        self.assertEqual(sim_ready.boot_task_processes(PS), ["SecuritySubscriber", "maild", "remotemanagementd"])

    def test_negative_control_nothing_running(self):
        self.assertEqual(sim_ready.boot_task_processes(f"{RUNTIME}/usr/libexec/healthd\n/usr/libexec/remotemanagementd"), [])


class WatchBootTasks(unittest.TestCase):
    def run_watch(self, booted_after, ps_output, stop=None, limit=1800.0):
        clock = FakeClock()

        def run(command, **_):
            if command[:3] == ["xcrun", "simctl", "list"]:
                state = "(Booted)" if clock.now >= booted_after else "(Shutdown)"
                return Completed(f"    iPad Pro 13-inch (M5) (UDID-1) {state}\n    Other (UDID-2) (Booted)\n")
            self.assertEqual(command, ["ps", "-axo", "comm="])
            return Completed(ps_output)

        out = io.StringIO()
        with redirect_stdout(out):
            sim_ready.watch_boot_tasks("UDID-1", run=run, clock=clock, sleep=clock.sleep, stop=stop, limit=limit)
        return out.getvalue().splitlines()

    def test_reports_each_checkpoint_after_the_device_is_booted(self):
        lines = self.run_watch(booted_after=40, ps_output=PS)
        self.assertTrue(lines[0].startswith("SIM BOOT TASKS UDID-1 booted seen at "))
        self.assertEqual(lines[1:], [
            f"SIM BOOT TASKS UDID-1 +{checkpoint} s after booted: running 3: SecuritySubscriber, maild, remotemanagementd"
            for checkpoint in sim_ready.BOOT_TASK_CHECKPOINTS])

    def test_reports_none_running(self):
        lines = self.run_watch(booted_after=0, ps_output=f"{RUNTIME}/usr/libexec/healthd\n")
        self.assertIn("SIM BOOT TASKS UDID-1 +480 s after booted: running 0: -", lines)

    def test_a_device_that_never_boots_ends_the_watch_silently(self):
        self.assertEqual(self.run_watch(booted_after=10 ** 9, ps_output=PS, limit=60), [])

    def test_a_set_stop_event_ends_the_watch(self):
        stop = threading.Event()
        stop.set()
        self.assertEqual(self.run_watch(booted_after=10 ** 9, ps_output=PS, stop=stop), [])


class QuietBootTasksInLibSh(unittest.TestCase):
    """lib.sh's ios_sim_quiet_boot_tasks and the ios_sim_boot call, with a stub xcrun on PATH."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.bin = Path(temporary.name)
        self.calls = self.bin / "calls.txt"
        stub = self.bin / "xcrun"
        # `simctl list --json` (ios_sim_resolve) names one shut-down device; bootstatus succeeds;
        # every `simctl spawn ... launchctl` call is logged and exits STUB_EXIT.
        stub.write_text(f"""#!/bin/bash
if [[ $1 == simctl && $2 == list ]]; then
  echo '{{"devices": {{"com.apple.CoreSimulator.SimRuntime.iOS-27-2": [{{"udid": "UDID-1", "name": "CI", "state": "Shutdown"}}]}}}}'
  exit 0
fi
if [[ $1 == simctl && $2 == spawn ]]; then echo "$*" >> {self.calls}; exit ${{STUB_EXIT:-0}}; fi
exit 0
""")
        stub.chmod(0o755)

    def bash(self, script, **env):
        environment = {**os.environ, "PATH": f"{self.bin}:{os.environ['PATH']}"}
        environment.pop("HERDR_IOS_QUIET_BOOT_TASKS", None)
        environment.update(env)
        return subprocess.run(["bash", "-c", f"source scripts/ios/lib.sh; {script}"], cwd=REPO, env=environment,
                              capture_output=True, text=True)

    def calls_made(self):
        return self.calls.read_text().splitlines() if self.calls.exists() else []

    def test_each_daemon_is_disabled_in_both_domains_and_booted_out(self):
        result = self.bash("ios_sim_quiet_boot_tasks UDID-1")
        self.assertEqual(result.returncode, 0)
        labels = ["com.apple.remotemanagementd", "com.apple.finhealthd", "com.apple.DeviceConfigurationAgent",
                  "com.apple.healthcontentd", "com.apple.email.maild", "com.apple.intelligencetasksd"]
        expected = []
        for label in labels:
            expected += [f"simctl spawn UDID-1 launchctl disable system/{label}",
                         f"simctl spawn UDID-1 launchctl disable user/foreground/{label}",
                         f"simctl spawn UDID-1 launchctl bootout system/{label}"]
        self.assertEqual(self.calls_made(), expected)
        self.assertIn("SIM BOOT TASK off com.apple.remotemanagementd disable:system/=0 disable:user/foreground/=0 "
                      "bootout:system/=0", result.stderr)

    def test_a_failing_launchctl_is_reported_and_does_not_fail(self):
        result = self.bash("ios_sim_quiet_boot_tasks UDID-1", STUB_EXIT="3")
        self.assertEqual(result.returncode, 0)
        self.assertIn("SIM BOOT TASK off com.apple.email.maild disable:system/=3 disable:user/foreground/=3 "
                      "bootout:system/=3", result.stderr)

    def test_ios_sim_boot_quiets_a_device_it_boots_when_asked(self):
        result = self.bash('ios_sim_boot UDID-1', HERDR_IOS_QUIET_BOOT_TASKS="1")
        self.assertEqual((result.returncode, result.stdout.strip()), (0, "UDID-1"))
        self.assertEqual(len(self.calls_made()), 18)

    def test_negative_control_ios_sim_boot_without_the_variable_leaves_the_daemons(self):
        result = self.bash('ios_sim_boot UDID-1')
        self.assertEqual((result.returncode, result.stdout.strip()), (0, "UDID-1"))
        self.assertEqual(self.calls_made(), [])
        self.assertNotIn("SIM BOOT TASK", result.stderr)


class Wiring(unittest.TestCase):
    """Only the hosted jobs' disposable-device wrappers ask lib.sh to quiet the device and watch it."""

    def test_both_wrappers_set_the_variable_and_start_the_watch_before_the_checks(self):
        for name in ["ci-sh1-152.py", "ci-smoke.py"]:
            source = (REPO / "scripts/ios" / name).read_text()
            with self.subTest(name):
                self.assertRegex(source, r"[\"']HERDR_IOS_QUIET_BOOT_TASKS[\"']: [\"']1[\"']")
                watch = source.index("start_boot_task_watch(device)")
                self.assertLess(watch, source.index("try:", watch))
                self.assertIn("stop_watch.set()", source[source.index("finally:", watch):])

    def test_the_owners_named_simulator_path_does_not_set_it(self):
        self.assertNotIn("HERDR_IOS_QUIET_BOOT_TASKS", (REPO / "scripts/ios/sim.sh").read_text())


if __name__ == "__main__":
    unittest.main()
