"""Exercise the environment adapter with GitHub-shaped input and inert secrets."""
import base64
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("testflight_secrets", Path(__file__).with_name("testflight-secrets.py"))
s = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(s)
ROOT = Path(__file__).resolve().parents[2]
FILES = ("distribution.p12", "app.mobileprovision", "AuthKey.p8")


class TestFlightSecretsTests(unittest.TestCase):
    def fixture(self, temporary):
        return {
            "ASC_KEY_ID": "FIXTUREKEY", "ASC_ISSUER_ID": "12345678-1234-1234-1234-123456789012",
            "ASC_KEY_P8": "-----BEGIN PRIVATE KEY-----\ninert-fixture\n-----END PRIVATE KEY-----\n",
            "ASC_APP_ID": "123456", "APPLE_TEAM_ID": "AL9567565U",
            "IOS_P12_BASE64": base64.b64encode(b"fixture-p12").decode(),
            "IOS_P12_PASSWORD": "fixture-password",
            "IOS_PROFILE_BASE64": base64.b64encode(b"fixture-profile").decode(),
            "RUNNER_TEMP": temporary, "GITHUB_RUN_NUMBER": "42",
            "GITHUB_REF": "refs/heads/main", "GITHUB_REF_TYPE": "branch", "GITHUB_REF_NAME": "main",
        }

    def run_main(self, env, stage):
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.dict(os.environ, env, clear=True), contextlib.redirect_stdout(stdout), \
                contextlib.redirect_stderr(stderr):
            status = s.main([stage])
        return status, stdout.getvalue() + stderr.getvalue()

    def test_validate_accepts_inputs_and_writes_nothing(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = self.fixture(temporary)
            status, output = self.run_main(env, "validate")
            self.assertEqual(status, 0, output)
            self.assertEqual(list(Path(temporary).iterdir()), [])
            for secret in (env["ASC_KEY_P8"], env["IOS_P12_PASSWORD"], env["IOS_P12_BASE64"]):
                self.assertNotIn(secret, output)

    def test_materialize_writes_private_files_without_logging_values(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = self.fixture(temporary)
            # The materialize step receives only the file inputs.
            only = {name: env[name] for name in (*s.FILE_INPUTS, "RUNNER_TEMP")}
            status, output = self.run_main(only, "materialize")
            self.assertEqual(status, 0, output)
            directory = Path(temporary) / "yebisu-testflight"
            self.assertEqual(directory.stat().st_mode & 0o777, 0o700)
            for name, expected in zip(FILES, (b"fixture-p12", b"fixture-profile", env["ASC_KEY_P8"].encode())):
                self.assertEqual((directory / name).read_bytes(), expected)
                self.assertEqual((directory / name).stat().st_mode & 0o777, 0o600)
            for secret in (env["ASC_KEY_P8"], env["IOS_P12_BASE64"]):
                self.assertNotIn(secret, output)

    def test_materialize_refuses_a_directory_planted_before_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = self.fixture(temporary)
            elsewhere = Path(temporary) / "elsewhere"
            elsewhere.mkdir()
            (Path(temporary) / "yebisu-testflight").symlink_to(elsewhere)
            status, output = self.run_main(env, "materialize")
            self.assertEqual(status, 1)
            self.assertIn("cannot materialize TestFlight inputs", output)
            self.assertEqual(list(elsewhere.iterdir()), [])

    def test_each_missing_secret_fails_before_creating_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            for name in s.REQUIRED:
                env = self.fixture(temporary)
                env[name] = ""
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "missing secrets: " + name):
                    s.validate(env, ROOT)
                self.assertEqual(list(Path(temporary).iterdir()), [])
            for name in s.FILE_INPUTS:
                env = self.fixture(temporary)
                env[name] = ""
                with self.subTest(materialize=name), self.assertRaisesRegex(ValueError, "missing secrets: " + name):
                    s.materialize(env)
                self.assertEqual(list(Path(temporary).iterdir()), [])

    def test_invalid_fields_do_not_leak_values(self):
        with tempfile.TemporaryDirectory() as temporary:
            for name in ("ASC_KEY_ID", "ASC_ISSUER_ID", "ASC_KEY_P8", "ASC_APP_ID",
                         "APPLE_TEAM_ID", "IOS_P12_BASE64", "IOS_PROFILE_BASE64",
                         "GITHUB_RUN_NUMBER", "GITHUB_REF"):
                for stage in ("validate", "materialize"):
                    env = self.fixture(temporary)
                    env[name] = "sensitive-invalid-fixture!"
                    status, output = self.run_main(env, stage)
                    with self.subTest(name=name, stage=stage):
                        self.assertNotIn(env[name], output)
                        if stage == "validate" or name in s.FILE_INPUTS:
                            self.assertEqual(status, 1)
                    shutil.rmtree(Path(temporary) / "yebisu-testflight", ignore_errors=True)
                    self.assertEqual(list(Path(temporary).iterdir()), [])

    def test_tags_and_other_branches_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            for ref in ("refs/tags/v0.1.0", "refs/heads/feature", "refs/pull/1/merge", ""):
                env = self.fixture(temporary)
                env["GITHUB_REF"] = ref
                with self.subTest(ref=ref), self.assertRaisesRegex(ValueError, "only from refs/heads/main"):
                    s.validate(env, ROOT)

    def test_cli_needs_a_stage_and_fails_without_traceback(self):
        script = str(Path(s.__file__))
        for args, status, message in (([], 2, "usage"), (["prepare"], 2, "usage"),
                                      (["validate"], 1, "environment testflight is missing secrets: ASC_KEY_ID"),
                                      (["materialize"], 1, "environment testflight is missing secrets: ASC_KEY_P8")):
            result = subprocess.run([sys.executable, "-I", "-B", script, *args],
                                    env={}, capture_output=True, text=True)
            with self.subTest(args=args):
                self.assertEqual(result.returncode, status)
                self.assertIn(message, result.stderr)
                self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
