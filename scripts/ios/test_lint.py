"""Check external UIKit linting without compiling or requiring Xcode."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest


SCRIPT = Path(__file__).with_name("lint.sh")


class LintTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="herdr-ios-lint-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        scripts = self.root / "scripts" / "ios"
        scripts.mkdir(parents=True)
        shutil.copyfile(SCRIPT, scripts / "lint.sh")
        (scripts / "test_fixture.py").write_text(textwrap.dedent('''
            from pathlib import Path
            import unittest

            class Fixture(unittest.TestCase):
                def test_isolated_script(self):
                    self.assertTrue(Path(__file__).with_name("lint.sh").is_file())
        '''))
        (scripts / "lib.sh").write_text(
            "ios_export_deployment_target() { export IPHONEOS_DEPLOYMENT_TARGET=17.0; }\n"
        )
        tools = self.root / "tools"
        tools.mkdir()
        self.log = self.root / "calls.jsonl"
        self.manifest = self.root / "cargo git checkout" / "Cargo.toml"
        cargo = tools / "cargo"
        cargo.write_text(f"#!{sys.executable}\n" + textwrap.dedent('''
            import json, os, sys
            args = sys.argv[1:]
            with open(os.environ["HERDR_IOS_LINT_TEST_LOG"], "a") as log:
                log.write(json.dumps(args) + "\\n")
            if args[0] == "metadata":
                packages = [{"name": "unrelated", "manifest_path": "/unrelated/Cargo.toml"}]
                if not os.environ.get("HERDR_IOS_LINT_TEST_MISSING"):
                    packages.append({
                        "name": "herdr-uikit",
                        "manifest_path": os.environ["HERDR_IOS_LINT_TEST_MANIFEST"],
                    })
                print(json.dumps({"packages": packages}))
        '''))
        cargo.chmod(0o755)
        self.env = {
            **os.environ,
            "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "HERDR_IOS_LINT_TEST_LOG": str(self.log),
            "HERDR_IOS_LINT_TEST_MANIFEST": str(self.manifest),
        }
        self.env.pop("CARGO_TARGET_DIR", None)

    def run_lint(self):
        return subprocess.run(
            ["bash", str(self.root / "scripts" / "ios" / "lint.sh")],
            env=self.env, text=True, capture_output=True, check=False,
        )

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_lints_both_application_targets_with_locked_resolution(self):
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 2)
        for call, target in zip(calls, ("aarch64-apple-ios", "aarch64-apple-ios-sim")):
            self.assertEqual(call[0], "clippy")
            self.assertIn("--locked", call)
            self.assertIn("--workspace", call)
            self.assertIn("--all-targets", call)
            self.assertEqual(call[call.index("--target") + 1], target)
            self.assertEqual(call[-3:], ["--", "-D", "warnings"])


if __name__ == "__main__":
    unittest.main()
