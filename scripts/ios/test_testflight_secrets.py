"""Exercise the environment adapter with GitHub-shaped input and inert secrets."""
import base64
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("testflight_secrets", Path(__file__).with_name("testflight-secrets.py"))
s = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(s)
ROOT = Path(__file__).resolve().parents[2]


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
            "GITHUB_REF_TYPE": "tag", "GITHUB_REF_NAME": "v0.1.0",
        }

    def test_ci_environment_becomes_private_files_without_logging_values(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = self.fixture(temporary)
            env["GITHUB_REF_TYPE"] = "branch"
            output = io.StringIO()
            with patch.dict(os.environ, env, clear=True), contextlib.redirect_stdout(output):
                self.assertEqual(s.main(), 0)
            directory = Path(temporary) / "yebisu-testflight"
            self.assertEqual(directory.stat().st_mode & 0o777, 0o700)
            for name, expected in (("distribution.p12", b"fixture-p12"),
                                   ("app.mobileprovision", b"fixture-profile"),
                                   ("AuthKey.p8", env["ASC_KEY_P8"].encode())):
                self.assertEqual((directory / name).read_bytes(), expected)
                self.assertEqual((directory / name).stat().st_mode & 0o777, 0o600)
            for secret in (env["ASC_KEY_P8"], env["IOS_P12_PASSWORD"], env["IOS_P12_BASE64"]):
                self.assertNotIn(secret, output.getvalue())

    def test_each_missing_secret_fails_before_creating_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            for name in s.REQUIRED:
                env = self.fixture(temporary)
                env[name] = ""
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "missing secrets: " + name):
                    s.prepare(env, ROOT)
                self.assertFalse((Path(temporary) / "yebisu-testflight").exists())

    def test_invalid_fields_do_not_leak_values(self):
        with tempfile.TemporaryDirectory() as temporary:
            for name in ("ASC_KEY_ID", "ASC_ISSUER_ID", "ASC_KEY_P8", "ASC_APP_ID",
                         "APPLE_TEAM_ID", "IOS_P12_BASE64", "IOS_PROFILE_BASE64",
                         "GITHUB_RUN_NUMBER", "GITHUB_REF_NAME"):
                env = self.fixture(temporary)
                env[name] = "sensitive-invalid-fixture!"
                output = io.StringIO()
                with self.subTest(name=name), patch.dict(os.environ, env, clear=True), contextlib.redirect_stderr(output):
                    self.assertEqual(s.main(), 1)
                self.assertNotIn(env[name], output.getvalue())
                self.assertFalse((Path(temporary) / "yebisu-testflight").exists())

    def test_empty_environment_cli_fails_with_clear_names_and_no_traceback(self):
        result = subprocess.run([sys.executable, "-I", "-B", str(Path(s.__file__))],
                                env={}, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("environment testflight is missing secrets: ASC_KEY_ID", result.stderr)
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
