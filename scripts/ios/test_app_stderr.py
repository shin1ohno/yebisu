"""The app's stderr copy reaches the artifact, and a lost-record line fails the check."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("app_stderr", Path(__file__).with_name("app_stderr.py"))
app_stderr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(app_stderr)


class AppStderrTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        self.documents, self.artifact = root / "Documents", root / "artifact"
        self.documents.mkdir()
        self.artifact.mkdir()
        self.log = self.documents / app_stderr.NAME

    def test_only_this_run_is_collected_into_the_artifact(self):
        self.log.write_bytes(b"app-stderr launch pid=1\nsmoke observation dropped: full\n")
        start = app_stderr.offset(self.documents)
        with self.log.open("ab") as log:
            log.write(b"app-stderr launch pid=2\nyebisu input click taps=1\n")
        text = app_stderr.collect(self.documents, self.artifact, start)
        self.assertEqual(text, "app-stderr launch pid=2\nyebisu input click taps=1\n")
        self.assertEqual((self.artifact / app_stderr.NAME).read_text(), text)
        app_stderr.require_no_loss(text)

    def test_every_lost_record_line_fails(self):
        for line in ("smoke observation dropped: full", "smoke observation dropped: disconnected",
                     "smoke observation writer: No space left on device (os error 28)",
                     "yebisu QA stderr capture failed: Bad file descriptor (os error 9)"):
            with self.subTest(line=line), self.assertRaisesRegex(ValueError, "lost observation records"):
                app_stderr.require_no_loss(f"app-stderr launch pid=3\n{line}\n")

    def test_a_missing_log_fails_instead_of_hiding_a_loss(self):
        self.assertEqual(app_stderr.offset(self.documents), 0)
        with self.assertRaisesRegex(ValueError, "not captured"):
            app_stderr.collect(self.documents, self.artifact)


if __name__ == "__main__":
    unittest.main()
