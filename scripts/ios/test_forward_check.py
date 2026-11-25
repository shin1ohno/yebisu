"""forward-check.py's own rules: its arguments, the private sshd it configures, and the key line."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("forward_check", Path(__file__).with_name("forward-check.py"))
forward_check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(forward_check)

LINE = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTY= herdr-ipad-simulator"


class Arguments(unittest.TestCase):
    def test_defaults_and_bounds(self):
        args = forward_check.parse_args([])
        self.assertEqual((args.port, args.keep), (8000, False))
        self.assertEqual(forward_check.parse_args(["--port", "3000", "--keep"]).port, 3000)
        for bad in (["--port", "0"], ["--port", "65536"], ["--port", "x"]):
            with self.assertRaises(SystemExit):
                forward_check.parse_args(bad)


class Sshd(unittest.TestCase):
    def test_loopback_public_keys_and_local_forwarding(self):
        config = forward_check.sshd_config(2222, Path("/k"), Path("/p"), Path("/a"))
        lines = config.splitlines()
        self.assertIn("ListenAddress 127.0.0.1", lines)
        self.assertIn("AllowTcpForwarding local", lines)
        self.assertIn("PasswordAuthentication no", lines)
        self.assertIn("AuthenticationMethods publickey", lines)
        self.assertIn("UsePAM no", lines)
        self.assertIn('AuthorizedKeysFile "/a"', lines)
        self.assertNotIn("restrict", config)
        self.assertFalse(any(line.startswith("ListenAddress 0") for line in lines))

    def test_paths_are_quoted(self):
        config = forward_check.sshd_config(1, Path('/a b/"k"'), Path("/p"), Path("/a"))
        self.assertIn('HostKey "/a b/\\"k\\""', config.splitlines())


class KeyLine(unittest.TestCase):
    def test_restrict_is_dropped_for_forwarding(self):
        self.assertEqual(forward_check.authorized_line("restrict " + LINE), LINE)
        self.assertEqual(forward_check.authorized_line(LINE + "\n"), LINE)

    def test_anything_else_is_refused(self):
        for bad in ("", "ssh-rsa AAAA x", "restrict,command=\"x\" " + LINE, LINE + " extra", "rm -rf /"):
            with self.assertRaises(ValueError, msg=bad):
                forward_check.authorized_line(bad)

    def test_the_last_printed_line_is_read(self):
        log = "noise\nFORWARD-KEY-LINE first\nmore\nFORWARD-KEY-LINE restrict %s\n" % LINE
        self.assertEqual(forward_check.key_line_from_log(log), "restrict " + LINE)
        with self.assertRaises(RuntimeError):
            forward_check.key_line_from_log("nothing here")


class Seeds(unittest.TestCase):
    def test_known_hosts_names_the_port(self):
        self.assertEqual(forward_check.known_hosts_line(2222, "ssh-ed25519 AAAAC3 me@mac\n"),
                         "[127.0.0.1]:2222 ssh-ed25519 AAAAC3\n")

    def test_saved_host_is_selected(self):
        import json
        saved = json.loads(forward_check.hosts_json("me@127.0.0.1:2222", "forward-qa"))
        self.assertEqual(saved, {"version": 1, "targets": ["me@127.0.0.1:2222"],
                                 "selected": "me@127.0.0.1:2222", "names": {"me@127.0.0.1:2222": "forward-qa"}})


if __name__ == "__main__":
    unittest.main()
