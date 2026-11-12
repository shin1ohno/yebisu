"""The Cargo simulator runner preserves selected raw OS environment names."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPT_DIR = Path(__file__).parent


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="herdr-ios-runner-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        shutil.copyfile(SCRIPT_DIR / "sim-runner.sh", self.root / "runner.sh")
        (self.root / "runner.sh").chmod(0o755)
        shutil.copyfile(SCRIPT_DIR / "sim-runner-env.py", self.root / "sim-runner-env.py")
        (self.root / "lib.sh").write_text(
            'HERDR_IOS_SIMULATOR=fixture-udid\n'
            'ios_sim_boot() { [[ $1 == fixture-udid ]] || return 1; printf "%s\\n" "$1"; }\n'
        )
        self.log = self.root / "received.json"
        self.receiver = self.root / "xcrun"
        self.receiver.write_text(
            f"#!{sys.executable}\n"
            "import json,os,sys\n"
            "names=[k for k in os.environ if k.startswith('SIMCTL_CHILD_')]\n"
            "with open(os.environ['HERDR_TEST_RUNNER_LOG'],'w') as f:\n"
            " json.dump({'args':sys.argv[1:],'env':{k:os.environ[k] for k in names}},f)\n"
            "if sys.argv[1:3] == ['simctl','spawn'] and not os.path.exists(sys.argv[4]): raise SystemExit(127)\n"
            "raise SystemExit(int(os.environ.get('HERDR_TEST_RECEIVER_STATUS','0')))\n"
        )
        self.receiver.chmod(0o755)
        self.shebang_path = self.root / "shebang-bin"
        self.shebang_path.mkdir()
        for command, target in (("bash", "/usr/bin/bash"), ("python3", sys.executable), ("xcrun", self.receiver), ("dirname", "/usr/bin/dirname")):
            (self.shebang_path / command).symlink_to(target)
        self.env = {
            "PATH": f"{self.root}{os.pathsep}/usr/bin:/bin",
            "HERDR_IOS_SIMULATOR": "fixture-udid",
            "HERDR_TEST_RUNNER_LOG": str(self.log),
            "CARGO_BIN_EXE_herdr-gpui": "/a path/端末\nsecond line",
            "CARGO_BIN_EXE_other_target": "",
            "RUST_BACKTRACE": "1 = $(touch never-run)",
            "HERDR_TEST_LITERAL": "semi; * ? `echo unsafe`\n値",
            "PRIVATE_NOT_A_TEST_VARIABLE": "private fixture",
        }
        self.binary = self.root / "test binary"
        self.binary.touch()

    def run_runner(self, shell, binary="/test binary", *args, env=None):
        return subprocess.run(
            [shell, str(self.root / "runner.sh"), binary, *args], cwd=self.root,
            env={**self.env, **(env or {})}, text=True, capture_output=True, check=False,
        )

    def test_selected_environment_and_arguments_reach_ordinary_runner(self):
        result = self.run_runner("/usr/bin/bash", str(self.binary), "argument with spaces", "日本語=\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "argument with spaces", "日本語=\n"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_herdr-gpui"], self.env["CARGO_BIN_EXE_herdr-gpui"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_other_target"], "")
        self.assertEqual(received["env"]["SIMCTL_CHILD_RUST_BACKTRACE"], self.env["RUST_BACKTRACE"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_HERDR_TEST_LITERAL"], self.env["HERDR_TEST_LITERAL"])
        self.assertNotIn("SIMCTL_CHILD_PRIVATE_NOT_A_TEST_VARIABLE", received["env"])
        self.assertFalse((self.root / "never-run").exists())

    def test_clipboard_branch_receives_selected_environment_and_arguments(self):
        (self.root / "clipboard-test-runner.sh").write_text(
            "#!/usr/bin/env bash\nexec xcrun clipboard \"$@\"\n"
        )
        clipboard_binary = self.root / "ios_clipboard-tests"
        clipboard_binary.touch()
        result = self.run_runner("/usr/bin/bash", "ios_clipboard-tests", "arg")
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["clipboard", "fixture-udid", str(clipboard_binary), "arg"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_herdr-gpui"], self.env["CARGO_BIN_EXE_herdr-gpui"])
        self.assertNotIn("SIMCTL_CHILD_PRIVATE_NOT_A_TEST_VARIABLE", received["env"])

    def test_production_env_bash_shebang_with_controlled_path(self):
        version = subprocess.run(["/usr/bin/bash", "--version"], text=True, capture_output=True, check=True).stdout.splitlines()[0]
        result = subprocess.run(
            [str(self.root / "runner.sh"), str(self.binary), "shebang-arg"], cwd=self.root,
            env={**self.env, "PATH": str(self.shebang_path)},
            text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "shebang-arg"])
        self.assertTrue(version.startswith("GNU bash, version 5.2."), version)

    def test_runner_error_paths_and_child_status(self):
        zero = subprocess.run(["/usr/bin/bash", str(self.root / "runner.sh")], env=self.env, capture_output=True)
        self.assertEqual(zero.returncode, 2)

        (self.root / "sim-runner-env.py").unlink()
        missing_helper = self.run_runner("/usr/bin/bash")
        self.assertEqual(missing_helper.returncode, 1)
        self.assertIn("missing environment helper", missing_helper.stderr)

        shutil.copyfile(SCRIPT_DIR / "sim-runner-env.py", self.root / "sim-runner-env.py")
        missing_binary = self.run_runner("/usr/bin/bash", str(self.root / "missing binary"))
        self.assertEqual(missing_binary.returncode, 127)

        status = self.run_runner("/usr/bin/bash", str(self.binary), env={"HERDR_TEST_RECEIVER_STATUS": "7"})
        self.assertEqual(status.returncode, 7)


if __name__ == "__main__":
    unittest.main()
