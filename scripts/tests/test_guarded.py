"""Exercise both disk-stop paths without consuming the host's free space."""
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import signal
from types import SimpleNamespace
import unittest

REPO = Path(__file__).resolve().parents[2]
RUNNER = r"""
import runpy, sys
from types import SimpleNamespace
from unittest.mock import patch
mode = sys.argv.pop(1)
count = 0
def disk(_):
    global count
    count += 1
    free = 7 if mode == "running" and count == 1 else 5
    return SimpleNamespace(f_bavail=free * 2**30, f_frsize=1)
sys.argv = sys.argv[1:]
with patch("os.statvfs", side_effect=disk):
    runpy.run_path(sys.argv[0], run_name="__main__")
"""

class GuardTests(unittest.TestCase):
    def run_guard(self, mode):
        temporary = tempfile.TemporaryDirectory(dir=REPO / "target")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        scripts = root / "scripts"
        scripts.mkdir()
        script = scripts / "guarded.py"
        shutil.copyfile(REPO / "scripts/guarded.py", script)
        marker = root / "started"
        child = "import os,time;from pathlib import Path;Path(" + repr(str(marker)) + ").write_text(str(os.getpid()));time.sleep(60)"
        result = subprocess.run([sys.executable, "-c", RUNNER, mode, str(script), sys.executable, "-c", child], capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 86, result.stderr)
        self.assertIn("below 6 GiB", (root / "target/disk-guard-fired").read_text())
        return marker

    def test_low_disk_refuses_to_start_the_command(self):
        self.assertFalse(self.run_guard("initial").exists())

    def test_running_command_is_stopped_when_space_drops(self):
        marker = self.run_guard("running")
        self.assertTrue(marker.exists())
        with self.assertRaises(ProcessLookupError):
            os.kill(int(marker.read_text()), 0)

    def test_ignoring_grandchild_dies_after_leader_exits_and_pipe_stays_open(self):
        with tempfile.TemporaryDirectory(dir=REPO / "target") as temp:
            root = Path(temp)
            (root / "scripts").mkdir()
            script = root / "scripts/guarded.py"
            shutil.copyfile(REPO / "scripts/guarded.py", script)
            leader = root / "leader"
            grandchild = root / "grandchild"
            heartbeat = root / "heartbeat"
            writer = f"""import os, signal, time
from pathlib import Path
signal.signal(signal.SIGTERM, signal.SIG_IGN)
Path({str(grandchild)!r}).write_text(str(os.getpid()))
with open({str(heartbeat)!r}, 'a', buffering=1) as stream:
    while True:
        stream.write("writer alive\\n")
        time.sleep(0.1)
"""
            parent = ("import os,subprocess,time;from pathlib import Path;"
                      f"Path({str(leader)!r}).write_text(str(os.getpid()));"
                      f"subprocess.Popen([{sys.executable!r},'-c',{writer!r}]);time.sleep(120)")
            started = time.monotonic()
            try:
                result = subprocess.run([sys.executable, "-c", RUNNER, "running", str(script), sys.executable, "-c", parent], capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 86, result.stderr)
                self.assertTrue(grandchild.exists(), result.stdout)
                self.assertIn('GUARD SIGKILL remaining process group', (root / 'target/disk.log').read_text())
                size = heartbeat.stat().st_size
                time.sleep(0.3)
                self.assertEqual(heartbeat.stat().st_size, size, 'grandchild is still writing')
                state = subprocess.run(['ps', '-p', grandchild.read_text(), '-o', 'stat='], capture_output=True, text=True)
                self.assertTrue(not state.stdout.strip() or state.stdout.strip().startswith('Z'), state.stdout)
                self.assertLess(time.monotonic() - started, 25)
                print(f'D3 PASS: leader={leader.read_text()} exited, TERM-ignoring stdout-holding grandchild={grandchild.read_text()} stopped; heartbeat_bytes={size}; state={state.stdout.strip()!r}; guard exit=86 in {time.monotonic()-started:.2f}s')
            finally:
                if leader.exists():
                    try:
                        os.killpg(int(leader.read_text()), signal.SIGKILL)
                    except (ProcessLookupError, PermissionError):
                        pass


class MachineFieldTests(unittest.TestCase):
    """Issue #256: each disk probe also records how late it ran, the load, swap and
    free memory; a missing value is written as such and never stops the guard."""

    def copy_guard(self):
        temporary = tempfile.TemporaryDirectory(dir=REPO / "target")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / "scripts").mkdir()
        script = root / "scripts/guarded.py"
        shutil.copyfile(REPO / "scripts/guarded.py", script)
        return root, script

    def load_guard(self):
        _, script = self.copy_guard()
        spec = importlib.util.spec_from_file_location("guarded_copy", script)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_probe_line_keeps_the_disk_fields_and_adds_the_machine_fields(self):
        root, script = self.copy_guard()
        result = subprocess.run([sys.executable, str(script), sys.executable, "-c", "pass"],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        line = (root / "target/disk.log").read_text().splitlines()[0]
        self.assertRegex(line, r"^disk \S+ free_bytes=\d+ free_GiB=[0-9.]+ late_s=0\.0 load1=[0-9.]+ "
                               r"(swap_used_MiB=[0-9.]+ memory_free_pct=\d+|memory=unavailable)$")

    def test_sysctl_values_are_parsed(self):
        guard = self.load_guard()
        output = "total = 2048.00M  used = 1059.25M  free = 988.75M  (encrypted)\n42\n"
        guard.subprocess = SimpleNamespace(run=lambda *a, **k: SimpleNamespace(returncode=0, stdout=output),
                                           SubprocessError=subprocess.SubprocessError)
        self.assertRegex(guard.machine(), r"^load1=[0-9.]+ swap_used_MiB=1059\.25 memory_free_pct=42$")

    def test_a_failing_or_odd_sysctl_is_recorded_as_unavailable(self):
        # Negative controls: no sysctl, a timeout, an error exit, an unexpected answer.
        def raises(error):
            def run(*_, **__):
                raise error
            return run
        answers = {
            "missing": raises(FileNotFoundError("sysctl")),
            "timeout": raises(subprocess.TimeoutExpired("sysctl", 2)),
            "error": lambda *a, **k: SimpleNamespace(returncode=1, stdout=""),
            "odd": lambda *a, **k: SimpleNamespace(returncode=0, stdout="no swap here\nlots\n"),
        }
        for name, run in answers.items():
            with self.subTest(name):
                guard = self.load_guard()
                guard.subprocess = SimpleNamespace(run=run, SubprocessError=subprocess.SubprocessError)
                self.assertRegex(guard.machine(), r"^load1=[0-9.]+ memory=unavailable$")

    def test_a_late_probe_records_its_delay(self):
        guard = self.load_guard()
        guard.machine = lambda: "load1=0.00 memory=unavailable"
        self.assertTrue(guard.probe(late=181.94))
        self.assertIn(" late_s=181.9 load1=0.00 memory=unavailable", (guard.ROOT / "disk.log").read_text())


if __name__ == "__main__":
    unittest.main()
