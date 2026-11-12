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
SYSTEM_BASH = Path("/usr/bin/bash") if Path("/usr/bin/bash").exists() else Path("/bin/bash")


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

    def probe_shell(self, label, shell_path):
        """Record the real shell identity before starting this runner row."""
        evidence_dir = os.environ.get("SIM_RUNNER_EVIDENCE_DIR")
        resolved_shell = os.path.realpath(shell_path)
        version = subprocess.run([resolved_shell, "--version"], text=True, capture_output=True, check=True).stdout.splitlines()[0]
        source_head = subprocess.run(
            ["git", "-C", str(SCRIPT_DIR.parents[1]), "rev-parse", "HEAD"],
            text=True, capture_output=True, check=True,
        ).stdout.strip()
        receipt = {
            "fixture": label,
            "shell_path": shell_path,
            "resolved_shell_path": resolved_shell,
            "shell_version": version,
            "source_head": source_head,
            "probe_order": "before_runner_execution",
        }
        if evidence_dir:
            destination = Path(evidence_dir)
            destination.mkdir(parents=True, exist_ok=True)
            (destination / f"{label}.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n")
        return receipt

    def save_receipt(self, label, receipt, received, exit_status):
        evidence_dir = os.environ.get("SIM_RUNNER_EVIDENCE_DIR")
        if not evidence_dir:
            return
        receipt["received"] = received
        receipt["runner_exit_status"] = exit_status
        destination = Path(evidence_dir)
        (destination / f"{label}.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n")

    def assert_received_environment(self, received):
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_herdr-gpui"], self.env["CARGO_BIN_EXE_herdr-gpui"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_CARGO_BIN_EXE_other_target"], "")
        self.assertEqual(received["env"]["SIMCTL_CHILD_RUST_BACKTRACE"], self.env["RUST_BACKTRACE"])
        self.assertEqual(received["env"]["SIMCTL_CHILD_HERDR_TEST_LITERAL"], self.env["HERDR_TEST_LITERAL"])
        self.assertNotIn("SIMCTL_CHILD_PRIVATE_NOT_A_TEST_VARIABLE", received["env"])
        self.assertFalse((self.root / "never-run").exists())

    def test_selected_environment_and_arguments_reach_ordinary_runner(self):
        shell_path = str(SYSTEM_BASH)
        receipt = self.probe_shell("ordinary-system-bash", shell_path)
        result = self.run_runner(shell_path, str(self.binary), "argument with spaces", "日本語=\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "argument with spaces", "日本語=\n"])
        self.assert_received_environment(received)
        self.save_receipt("ordinary-system-bash", receipt, received, result.returncode)

        receipt = self.probe_shell("ordinary-system-bash-exit7", shell_path)
        status = self.run_runner(shell_path, str(self.binary), "exit-seven", env={"HERDR_TEST_RECEIVER_STATUS": "7"})
        self.assertEqual(status.returncode, 7, status.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "exit-seven"])
        self.assert_received_environment(received)
        self.save_receipt("ordinary-system-bash-exit7", receipt, received, status.returncode)

    def test_clipboard_branch_receives_selected_environment_and_arguments(self):
        (self.root / "clipboard-test-runner.sh").write_text(
            "#!/usr/bin/env bash\nexec xcrun clipboard \"$@\"\n"
        )
        clipboard_binary = self.root / "ios_clipboard-tests"
        clipboard_binary.touch()
        shell_path = str(SYSTEM_BASH)
        receipt = self.probe_shell("clipboard-system-bash", shell_path)
        result = self.run_runner(shell_path, "ios_clipboard-tests", "arg")
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        resolved_clipboard_binary = os.path.realpath(clipboard_binary)
        self.assertEqual(received["args"], ["clipboard", "fixture-udid", resolved_clipboard_binary, "arg"])
        self.assert_received_environment(received)
        self.save_receipt("clipboard-system-bash", receipt, received, result.returncode)

        receipt = self.probe_shell("clipboard-env-bash", str(self.shebang_path / "bash"))
        result = subprocess.run(
            [str(self.root / "runner.sh"), "ios_clipboard-tests", "shebang-arg"], cwd=self.root,
            env={**self.env, "PATH": str(self.shebang_path)}, text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        shebang_received = json.loads(self.log.read_text())
        self.assertEqual(shebang_received["args"], ["clipboard", "fixture-udid", resolved_clipboard_binary, "shebang-arg"])
        self.assert_received_environment(shebang_received)
        self.save_receipt("clipboard-env-bash", receipt, shebang_received, result.returncode)

        receipt = self.probe_shell("clipboard-system-bash-exit7", shell_path)
        system_status = self.run_runner(
            str(SYSTEM_BASH), "ios_clipboard-tests", "exit-seven", env={"HERDR_TEST_RECEIVER_STATUS": "7"}
        )
        self.assertEqual(system_status.returncode, 7)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["clipboard", "fixture-udid", resolved_clipboard_binary, "exit-seven"])
        self.assert_received_environment(received)
        self.save_receipt("clipboard-system-bash-exit7", receipt, received, system_status.returncode)

        receipt = self.probe_shell("clipboard-env-bash-exit7", str(self.shebang_path / "bash"))
        shebang_status = subprocess.run(
            [str(self.root / "runner.sh"), "ios_clipboard-tests", "exit-seven-shebang"], cwd=self.root,
            env={**self.env, "PATH": str(self.shebang_path), "HERDR_TEST_RECEIVER_STATUS": "7"},
            text=True, capture_output=True, check=False,
        )
        self.assertEqual(shebang_status.returncode, 7, shebang_status.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["clipboard", "fixture-udid", resolved_clipboard_binary, "exit-seven-shebang"])
        self.assert_received_environment(received)
        self.save_receipt("clipboard-env-bash-exit7", receipt, received, shebang_status.returncode)

    def test_production_env_bash_shebang_with_controlled_path(self):
        shebang_bash = str(self.shebang_path / "bash")
        receipt = self.probe_shell("ordinary-env-bash", shebang_bash)
        result = subprocess.run(
            [str(self.root / "runner.sh"), str(self.binary), "shebang-arg"], cwd=self.root,
            env={**self.env, "PATH": str(self.shebang_path)},
            text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "shebang-arg"])
        self.assert_received_environment(received)
        self.assertIn("bash", receipt["shell_version"].lower(), receipt["shell_version"])
        self.save_receipt("ordinary-env-bash", receipt, received, result.returncode)

        receipt = self.probe_shell("ordinary-env-bash-exit7", shebang_bash)
        status = subprocess.run(
            [str(self.root / "runner.sh"), str(self.binary), "exit-seven-shebang"], cwd=self.root,
            env={**self.env, "PATH": str(self.shebang_path), "HERDR_TEST_RECEIVER_STATUS": "7"},
            text=True, capture_output=True, check=False,
        )
        self.assertEqual(status.returncode, 7, status.stderr)
        received = json.loads(self.log.read_text())
        self.assertEqual(received["args"], ["simctl", "spawn", "fixture-udid", str(self.binary), "exit-seven-shebang"])
        self.assert_received_environment(received)
        self.save_receipt("ordinary-env-bash-exit7", receipt, received, status.returncode)

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
        self.save_receipt("ordinary-system-bash-exit7", str(SYSTEM_BASH), json.loads(self.log.read_text()))


if __name__ == "__main__":
    unittest.main()
