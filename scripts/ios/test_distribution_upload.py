"""Offline upload contracts; fixture IPA/key bytes only, never Apple transport."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile
from distribution_fixtures import ICON_INFO, ICON_FILES

# Shaped like the ES256 JWT altool signs with the .p8; derived, so no injected value matches it.
JWT = ("eyJhbGciOiJFUzI1NiIsImtpZCI6IkZJWFRVUkVLRVkiLCJ0eXAiOiJKV1QifQ"
       ".eyJpc3MiOiJmaXh0dXJlIiwiYXVkIjoiYXBwc3RvcmVjb25uZWN0LXYxIn0"
       ".Zml4dHVyZS1zaWduYXR1cmUtbm90LXJlYWwtZml4dHVyZS1zaWduYXR1cmUtbm90LXJlYWw")
OPAQUE = "opaque-fixture-access-token-0123456789"
UUID = "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"
# Review N1 forms: split JWTs (mid-header, after the first dot, CRLF, indented),
# a JSON \n escape or an underscore before the JWT, an escaped dot, bearers.
TOKEN_LINES = [JWT, JWT[:30], JWT[30:], JWT[:JWT.index(".") + 1] + "\r", JWT[JWT.index(".") + 1:],
               "    " + JWT[:50], "    " + JWT[50:], "line1\\n" + JWT, "jwt_" + JWT,
               JWT.replace(".", "\\."), "Authorization: Bearer", JWT, "authorization: bearer " + OPAQUE]


def assert_no_jwt_piece(test, output):
    pieces = [JWT[start:start + 12] for start in range(len(JWT) - 11)]
    test.assertTrue(pieces, "positive control: the fixture JWT has pieces")
    for piece in pieces:
        test.assertNotIn(piece, output)


SPEC = importlib.util.spec_from_file_location("distribution", Path(__file__).with_name("distribution.py"))
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)


class UploadTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.ipa = self.root / "fixture package.ipa"
        self.info = dict(CFBundleIdentifier=d.BUNDLE, CFBundleSupportedPlatforms=["iPhoneOS"],
                         CFBundleVersion="12.3", CFBundleShortVersionString="1.2.0", **ICON_INFO)
        self.key = self.root / "injected key with spaces.p8"
        self.key.write_bytes(b"FIXTURE ONLY; NOT A PRIVATE KEY")
        self.env = dict(ASC_KEY_FILE=str(self.key), ASC_KEY_ID="FIXTUREKEY",
            ASC_ISSUER_ID="00000000-0000-0000-0000-000000000000", ASC_APP_ID="1234567890")
        self.write_ipa(self.info)

    def write_ipa(self, info, extra=None):
        with zipfile.ZipFile(self.ipa, "w") as archive:
            archive.writestr("Payload/fixture.app/Info.plist", plistlib.dumps(info))
            for name, content in ICON_FILES.items():
                archive.writestr("Payload/fixture.app/" + name, content)
            if extra:
                archive.writestr(extra, plistlib.dumps(info))

    def test_upload_runs_fake_transport_with_exact_metadata_and_original_key_path(self):
        fakebin = self.root / "tools"
        fakebin.mkdir()
        fake = fakebin / "xcrun"
        fake.write_text("#!/usr/bin/python3\nimport json,os,sys\n"
                        "open(os.environ['FIXTURE_ARGV'],'w').write(json.dumps(sys.argv[1:]))\n")
        fake.chmod(0o755)
        log = self.root / "argv.json"
        original = self.key.read_bytes()
        with patch.dict(os.environ, dict(self.env, PATH=str(fakebin), FIXTURE_ARGV=str(log)), clear=True):
            d.upload_ipa(self.ipa)
        argv = json.loads(log.read_text())
        self.assertEqual(argv[:3], ["altool", "--upload-package", str(self.ipa.resolve())])
        for flag, value in [("--p8-file-path", str(self.key.resolve())), ("--api-key", "FIXTUREKEY"),
                ("--api-issuer", self.env["ASC_ISSUER_ID"])]:
            self.assertEqual(argv[argv.index(flag) + 1], value)
        self.assertEqual(self.key.read_bytes(), original)
        self.assertEqual(list(self.root.rglob("*.p8")), [self.key])
        for flag in ("--file", "--type", "--apiKey", "--apiIssuer", "-t", "--apple-id", "--bundle-id", "--bundle-version", "--bundle-short-version-string"):
            self.assertNotIn(flag, argv)

    def test_missing_upload_credentials_exit_nonzero(self):
        # A structurally valid IPA gets past metadata and reaches credential validation.
        self.assertEqual(d.ipa_metadata(self.ipa)['CFBundleVersion'], '12.3')
        with patch.dict(os.environ, {}, clear=True), patch.object(d, 'run') as runner, \
                patch('sys.argv', ['distribution.py', 'upload', '--ipa', str(self.ipa)]), \
                contextlib.redirect_stderr(io.StringIO()) as stderr:
            self.assertEqual(d.main(), 1)
        self.assertIn('ASC_KEY_FILE must name an existing secret file', stderr.getvalue())
        self.assertNotIn('IPA is missing', stderr.getvalue())
        runner.assert_not_called()

    def test_iconless_ipa_is_rejected_before_transport(self):
        for missing in ICON_FILES:
            with zipfile.ZipFile(self.ipa, 'w') as archive:
                archive.writestr('Payload/fixture.app/Info.plist', plistlib.dumps(self.info))
                for name, content in ICON_FILES.items():
                    if name != missing:
                        archive.writestr('Payload/fixture.app/' + name, content)
            with self.subTest(missing=missing), patch.object(d, 'run') as runner:
                with self.assertRaisesRegex(ValueError, 'AppIcon'):
                    d.upload_ipa(self.ipa)
                runner.assert_not_called()

    def test_optional_provider_and_invalid_auth_metadata(self):
        provider = "11111111-1111-1111-1111-111111111111"
        with patch.dict(os.environ, dict(self.env, ASC_PROVIDER_PUBLIC_ID=provider), clear=True), patch.object(d, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as run:
            d.upload_ipa(self.ipa)
            self.assertEqual(run.call_args.args[0][-2:], ["--provider-public-id", provider])
        for update in [dict(ASC_KEY_FILE="/missing.p8"), dict(ASC_KEY_ID="bad; shell"),
                dict(ASC_ISSUER_ID="not-a-uuid"), dict(ASC_APP_ID=""),
                dict(ASC_PROVIDER_PUBLIC_ID="not-a-uuid")]:
            with self.subTest(update=update), patch.dict(os.environ, dict(self.env, **update), clear=True), patch.object(d, "run") as run:
                with self.assertRaises(ValueError):
                    d.upload_ipa(self.ipa)
                run.assert_not_called()

    def test_invalid_package_never_reaches_transport(self):
        for update in [dict(CFBundleIdentifier="other.app"), dict(CFBundleVersion=""),
                dict(CFBundleShortVersionString="1; shell"), dict(CFBundleSupportedPlatforms=["iPhoneSimulator"]),
                {"CFBundleIcons~ipad": "invalid dictionary"}, {"CFBundleIcons~ipad": {"CFBundlePrimaryIcon": []}},
                {"CFBundleIcons~ipad": {"CFBundlePrimaryIcon": {"CFBundleIconName": "AppIcon", "CFBundleIconFiles": "icon"}}}]:
            self.write_ipa(dict(self.info, **update))
            with self.subTest(update=update), patch.dict(os.environ, self.env, clear=True), patch.object(d, "run") as run:
                with self.assertRaises(ValueError):
                    d.upload_ipa(self.ipa)
                run.assert_not_called()
        self.write_ipa(["invalid plist root"])
        with self.assertRaises(ValueError):
            d.ipa_metadata(self.ipa)
        self.write_ipa(self.info, "Payload/second.app/Info.plist")
        with self.assertRaises(ValueError):
            d.ipa_metadata(self.ipa)
        with zipfile.ZipFile(self.ipa, "w") as archive:
            archive.writestr("Payload/fixture.app/Info.plist", b"x" * 1_048_577)
        with self.assertRaises(ValueError):
            d.ipa_metadata(self.ipa)

    def fake_altool(self, status):
        """An xcrun that replays altool-shaped JSON and stderr, carrying every token form."""
        fakebin = self.root / "tools"
        fakebin.mkdir(exist_ok=True)
        fake = fakebin / "xcrun"
        fake.write_text("#!/usr/bin/python3\nimport os,sys\n"
                        "sys.stdout.write(open(os.environ['FIXTURE_STDOUT']).read())\n"
                        "sys.stderr.write(open(os.environ['FIXTURE_STDERR']).read())\n"
                        "sys.exit(int(os.environ['FIXTURE_STATUS']))\n")
        fake.chmod(0o755)
        echoed = [self.env["ASC_KEY_ID"], self.env["ASC_ISSUER_ID"], self.env["ASC_APP_ID"],
                  str(self.key.resolve()), self.key.read_text()]
        body = {"tool-version": "fixture", "os-version": "fixture", "echo": echoed,
                "headers": "Authorization: Bearer " + JWT, "token": JWT}
        if status:
            body["product-errors"] = [{"code": -19232, "message": "FIXTURE_UPLOAD_REJECTED", "userInfo": {
                "NSLocalizedDescription": "Request headers:\nAuthorization: Bearer " + JWT,
                "NSLocalizedFailureReason": "line1\n" + JWT,
                "NSLocalizedRecoverySuggestion": "jwt_" + JWT + "\nFIXTURE_SUGGESTION_KEPT",
                "AuthorizationHeader": "Bearer " + JWT},
                "underlying-errors": [{"code": -1, "message": "FIXTURE_UNDERLYING\n" + JWT[:30] + "\n" + JWT[30:]}]}]
        else:
            body["success-message"] = "FIXTURE_UPLOAD_ACCEPTED"
        stdout, stderr = self.root / "altool-stdout.json", self.root / "altool-stderr.txt"
        stdout.write_text(json.dumps(body))
        stderr.write_text("\n".join(["FIXTURE_ALTOOL_STDERR " + self.env["ASC_KEY_ID"], *TOKEN_LINES,
                                     "FIXTURE_STDERR_TAIL " + UUID]) + "\n")
        return dict(PATH=str(fakebin), FIXTURE_STDOUT=str(stdout), FIXTURE_STDERR=str(stderr),
                    FIXTURE_STATUS=str(status))

    def run_upload(self, status):
        env = dict(self.env, **self.fake_altool(status))
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.dict(os.environ, env, clear=True), \
                patch("sys.argv", ["distribution.py", "upload", "--ipa", str(self.ipa)]), \
                contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = d.main()
        return code, stdout.getvalue() + stderr.getvalue()

    def assert_no_secret(self, output):
        for value in (self.env["ASC_KEY_ID"], self.env["ASC_ISSUER_ID"], self.env["ASC_APP_ID"],
                      str(self.key.resolve()), str(self.key.resolve()).replace("/", "\\/"),
                      self.key.read_text(), OPAQUE):
            self.assertNotIn(value, output)
        assert_no_jwt_piece(self, output)
        self.assertIn(d.WITHHELD, output, "positive control: token lines were seen and withheld")
        self.assertIn("FIXTURE_ALTOOL_STDERR [redacted]", output)
        self.assertIn("FIXTURE_STDERR_TAIL " + UUID, output)

    def test_upload_success_shows_altool_json_and_stderr_without_secrets(self):
        code, output = self.run_upload(0)
        self.assertEqual(code, 0, output)
        self.assertIn("[xcrun altool] exit status 0", output)
        self.assertIn('"success-message": "FIXTURE_UPLOAD_ACCEPTED"', output)
        self.assertIn('"tool-version": "fixture"', output)
        self.assert_no_secret(output)

    def test_upload_failure_shows_altool_json_and_stderr_without_secrets(self):
        code, output = self.run_upload(1)
        self.assertEqual(code, 1)
        self.assertIn("[xcrun altool] exit status 1", output)
        for kept in ('"code": -19232', '"message": "FIXTURE_UPLOAD_REJECTED"', '"code": -1',
                     "FIXTURE_UNDERLYING", "FIXTURE_SUGGESTION_KEPT", "Request headers:", "line1"):
            self.assertIn(kept, output)
        self.assertNotIn("AuthorizationHeader", output)
        self.assertIn("external command failed with exit status 1", output)
        self.assert_no_secret(output)

    def test_each_token_form_is_withheld_and_plain_lines_stay(self):
        forms = {"split mid-header": JWT[:30] + "\n" + JWT[30:],
                 "split after the first dot, CRLF": JWT[:JWT.index(".") + 1] + "\r\n" + JWT[JWT.index(".") + 1:],
                 "indented continuation": "    " + JWT[:50] + "\n    " + JWT[50:],
                 "JSON newline escape": "line1\\n" + JWT, "underscore prefix": "jwt_" + JWT,
                 "escaped dot": JWT.replace(".", "\\."), "bearer then newline": "Bearer\n" + JWT,
                 "lowercase bearer": "authorization: bearer " + OPAQUE}
        for name, text in forms.items():
            with self.subTest(form=name):
                shown = d.redact(d.altool_view("stderr", text + "\nplain diagnostic " + UUID), set())
                assert_no_jwt_piece(self, shown)
                self.assertNotIn(OPAQUE, shown)
                self.assertIn(d.WITHHELD, shown)
                self.assertIn("plain diagnostic " + UUID, shown)

    def test_transport_failure_is_nonzero_and_does_not_expose_arguments(self):
        with patch.dict(os.environ, self.env, clear=True), \
                patch.object(d, "run", side_effect=subprocess.CalledProcessError(1, ["PRIVATE_SENTINEL"])), \
                patch("sys.argv", ["distribution.py", "upload", "--ipa", str(self.ipa.resolve())]), \
                contextlib.redirect_stderr(io.StringIO()) as stderr:
            self.assertEqual(d.main(), 1)
        self.assertNotIn("PRIVATE_SENTINEL", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
