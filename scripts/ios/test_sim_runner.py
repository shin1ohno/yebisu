"""The Cargo runner preserves OS environment names that are not shell identifiers."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class RunnerTests(unittest.TestCase):
    def test_cargo_binary_names_and_values_reach_the_selected_simulator(self):
        with tempfile.TemporaryDirectory(prefix="herdr-ios-runner-") as directory:
            root = Path(directory)
            shutil.copyfile(Path(__file__).with_name("sim-runner.sh"), root / "runner.sh")
            (root / "lib.sh").write_text(
                'HERDR_IOS_SIMULATOR=fixture-udid\n'
                'ios_sim_boot() { [[ $1 == fixture-udid ]] || return 1; printf "%s\\n" "$1"; }\n'
            )
            log = root / "received.json"
            xcrun = root / "xcrun"
            xcrun.write_text(f"#!{sys.executable}\n" +
                             "import json,os,sys\n" +
                             "with open(os.environ['HERDR_TEST_RUNNER_LOG'],'w') as f:\n" +
                             " json.dump({'args':sys.argv[1:],'env':dict(os.environ)},f)\n")
            xcrun.chmod(0o755)
            value = "/a path/with spaces/端末\nsecond line"
            env = {
                **os.environ,
                "PATH": f"{root}{os.pathsep}{os.environ['PATH']}",
                "CARGO_BIN_EXE_herdr-gpui": value,
                "RUST_BACKTRACE": "1",
                "HERDR_TEST_RUNNER_LOG": str(log),
                "PRIVATE_NOT_A_TEST_VARIABLE": "do not forward",
            }
            result = subprocess.run(
                ["bash", str(root / "runner.sh"), "/test binary", "argument with spaces"],
                env=env, text=True, capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            received = json.loads(log.read_text())
            self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", "/test binary", "argument with spaces"])
            self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_herdr-gpui"], value)
            self.assertEqual(received["env"]["SIMCTL_CHILD_RUST_BACKTRACE"], "1")
            self.assertNotIn("SIMCTL_CHILD_PRIVATE_NOT_A_TEST_VARIABLE", received["env"])


if __name__ == "__main__":
    unittest.main()
