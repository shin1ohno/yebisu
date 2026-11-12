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
SYSTEM_BASH = Path("/bin/bash") if Path("/bin/bash").exists() else Path(shutil.which("bash") or "/usr/bin/bash")


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
        for command, target in (("bash", SYSTEM_BASH), ("python3", sys.executable), ("xcrun", self.receiver), ("dirname", shutil.which("dirname") or "/usr/bin/dirname")):
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

    def save_receipt(self, label, shell_path, received):
        evidence_dir = os.environ.get("SIM_RUNNER_EVIDENCE_DIR")
        if not evidence_dir:
            return
        resolved_shell = os.path.realpath(shell_path)
        version = subprocess.run([resolved_shell, "--version"], text=True, capture_output=True, check=True).stdout.splitlines()[0]
        output = {
            "fixture": label,
            "shell_path": shell_path,
            "resolved_shell_path": resolved_shell,
            "shell_version": version,
            "received": received,
        }
        destination = Path(evidence_dir)
        destination.mkdir(parents=True, exist_ok=True)
        (destination / f"{label}.json").write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")

    def test_selected_environment_and_arguments_reach_ordinary_runner(self):
        result = self.run_runner(str(SYSTEM_BASH), str(self.binary), "argument with spaces", "日本語=\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "argument with spaces", "日本語=\n"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_herdr-gpui"], self.env["CARGO_BIN_EXE_herdr-gpui"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_other_target"], "")
        self.assertEqual(received["env"]["SIMCTL_CHILD_RUST_BACKTRACE"], self.env["RUST_BACKTRACE"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_HERDR_TEST_LITERAL"], self.env["HERDR_TEST_LITERAL"])
        self.assertNotIn("SIMCTL_CHILD_PRIVATE_NOT_A_TEST_VARIABLE", received["env"])
        self.assertFalse((self.root / "never-run").exists())
        self.save_receipt("ordinary-system-bash", str(SYSTEM_BASH), received)

    def test_clipboard_branch_receives_selected_environment_and_arguments(self):
        (self.root / "clipboard-test-runner.sh").write_text(
            "#!/usr/bin/env bash\nexec xcrun clipboard \"$@\"\n"
        )
        clipboard_binary = self.root / "ios_clipboard-tests"
        clipboard_binary.touch()
        result = self.run_runner(str(SYSTEM_BASH), "ios_clipboard-tests", "arg")
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["clipboard", "fixture-udid", str(clipboard_binary), "arg"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_herdr-gpui"], self.env["CARGO_BIN_EXE_herdr-gpui"])
        self.assertNotIn("SIMCTL_CHILD_PRIVATE_NOT_A_TEST_VARIABLE", received["env"])
        self.save_receipt("clipboard-system-bash", str(SYSTEM_BASH), received)

    def test_production_env_bash_shebang_with_controlled_path(self):
        shebang_bash = str(self.shebang_path / "bash")
        result = subprocess.run(
            [str(self.root / "runner.sh"), str(self.binary), "shebang-arg"], cwd=self.root,
            env={**self.env, "PATH": str(self.shebang_path)},
            text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "shebang-arg"])
        version = subprocess.run([os.path.realpath(shebang_bash), "--version"], text=True, capture_output=True, check=True).stdout.splitlines()[0]
        self.assertIn("bash", version.lower(), version)
        self.save_receipt("env-bash-shebang", shebang_bash, received)

    def test_runner_error_paths_and_child_status(self):
        zero = subprocess.run([str(SYSTEM_BASH), str(self.root / "runner.sh")], env=self.env, capture_output=True)
        self.assertEqual(zero.returncode, 2)

        (self.root / "sim-runner-env.py").unlink()
        missing_helper = self.run_runner(str(SYSTEM_BASH))
        self.assertEqual(missing_helper.returncode, 1)
        self.assertIn("missing environment helper", missing_helper.stderr)

        shutil.copyfile(SCRIPT_DIR / "sim-runner-env.py", self.root / "sim-runner-env.py")
        missing_binary = self.run_runner(str(SYSTEM_BASH), str(self.root / "missing binary"))
        self.assertEqual(missing_binary.returncode, 127)

        status = self.run_runner(str(SYSTEM_BASH), str(self.binary), env={"HERDR_TEST_RECEIVER_STATUS": "7"})
        self.assertEqual(status.returncode, 7)


if __name__ == "__main__":
    unittest.main()
