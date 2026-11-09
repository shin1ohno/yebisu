"""Exercise both disk-stop paths without consuming the host's free space."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
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
        result = subprocess.run([sys.executable, "-c", RUNNER, mode, str(script), sys.executable, "-c", child], capture_output=True, text=True, timeout=20)
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

if __name__ == "__main__":
    unittest.main()
