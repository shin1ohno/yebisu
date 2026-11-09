"""Exercise both disk-stop paths without consuming the host's free space."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import signal
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


if __name__ == "__main__":
    unittest.main()
