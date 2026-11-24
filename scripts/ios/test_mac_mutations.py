"""Each gate in mac.sh, and provision.sh's --register-device, is what refuses.

    python3 -I -B -m unittest discover -s scripts/ios -p 'test_mac_mutations.py'

Every gate sits between `# gate:NAME begin` and `# gate:NAME end`. Its
scenario runs on a copy as written, where the refusal must hold (the positive
control), and on a copy with that region deleted, where the same run must get
past the refusal to the step the gate protects. The pair must occur exactly
once, and a mutant must pass `bash -n`, so a renamed gate or a broken deletion
fails here instead of looking like a proof. Pure file edits; no build.
"""

import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import unittest

HERE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


mac_tests = load("test_mac")
provision_tests = load("test_provision_destination")


def without_gate(text, gate):
    lines = text.splitlines(keepends=True)
    begins = [i for i, line in enumerate(lines) if line.strip() == f"# gate:{gate} begin"]
    ends = [i for i, line in enumerate(lines) if line.strip() == f"# gate:{gate} end"]
    if len(begins) != 1 or len(ends) != 1 or begins[0] > ends[0]:
        raise AssertionError(f"gate {gate}: markers found {len(begins)}/{len(ends)} times, not once each")
    return "".join(lines[:begins[0]] + lines[ends[0] + 1:])


def syntax_ok(test, script):
    checked = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True, check=False)
    test.assertEqual(checked.returncode, 0, f"mutant is not valid bash: {checked.stderr}")


# gate: (prepare the refused run -> its arguments, step, status, whether the
# run got past the gate to what it protects).
MAC_GATES = {
    "bundle-id": (lambda t: ("--bundle-id", "com.example.other"), "options", 2,
                  lambda t, result: bool(t.calls("provision.sh"))),
    "identity": (lambda t: t.env.update(MAC_TEST_VALID=mac_tests.NONE_VALID) or (), "preflight", 1,
                 lambda t, result: bool(t.calls("provision.sh"))),
    "registration": (lambda t: t.env.update(MAC_TEST_PROVISION_STATUS="1",
                                            MAC_TEST_PROVISION_STDERR=mac_tests.UNREGISTERED) or (),
                     "register", 3,
                     lambda t, result: result.returncode == 1 and result.stderr.splitlines()[-1].startswith(
                         "error: just mac failed at step provision:")),
    "profile-udid": (lambda t: t.env.update(MAC_TEST_PROFILE=str(t.write_profile(
                         t.root / "unlisted.mobileprovision", devices=[mac_tests.OTHER_DEVICE]))) or (),
                     "profile-check", 1, lambda t, result: bool(t.calls("bundle.sh"))),
    "bundle-id-built": (lambda t: t.env.update(MAC_TEST_APP=str(t.make_app(
                            t.root / "other" / "yebisu.app", bundle_id="com.example.other"))) or (),
                        "bundle-check", 2, lambda t, result: bool(t.opened("ipa"))),
    "embedded-profile": (lambda t: ("--app", str(t.make_app(
                             t.root / "given" / "yebisu.app", devices=[mac_tests.OTHER_DEVICE]))),
                         "bundle-check", 1, lambda t, result: bool(t.opened("ipa"))),
}


@unittest.skipUnless(sys.platform == "darwin", "mac.sh reads bundles with plutil and zips with ditto")
class MacGateMutationTests(mac_tests.Harness, unittest.TestCase):
    def check(self, gate):
        prepare, step, status, got_past = MAC_GATES[gate]
        mutant_text = without_gate(self.script.read_text(), gate)
        args = prepare(self)
        result = self.run_mac(*args)
        self.assert_refused(result, step, status)
        self.assertFalse(got_past(self, result), "the positive control already got past the gate")
        mutant = self.scripted_tree(self.root / f"mutant-{gate}")
        mutant.write_text(mutant_text)
        syntax_ok(self, mutant)
        self.log.unlink(missing_ok=True)
        result = self.run_mac(*args, script=mutant)
        self.assertTrue(got_past(self, result), f"without gate {gate} the run still stopped:\n{result.stderr}")

    def test_bundle_id_gate(self):
        self.check("bundle-id")

    def test_identity_gate(self):
        self.check("identity")

    def test_registration_gate(self):
        self.check("registration")

    def test_profile_udid_gate(self):
        self.check("profile-udid")

    def test_bundle_id_built_gate(self):
        self.check("bundle-id-built")

    def test_embedded_profile_gate(self):
        self.check("embedded-profile")

    def test_every_gate_marker_in_mac_sh_has_a_mutation(self):
        text = (HERE / "mac.sh").read_text()
        self.assertEqual(set(re.findall(r"# gate:([a-z-]+) begin", text)), set(MAC_GATES))
        self.assertEqual(set(re.findall(r"# gate:([a-z-]+) end", text)), set(MAC_GATES))


@unittest.skipUnless(sys.platform == "darwin", "provision.sh reads Info.plist with plutil")
class ProvisionGateMutationTests(unittest.TestCase):
    def setUp(self):
        # The fake xcodegen, xcodebuild and security of test_provision_destination.
        provision_tests.ProvisionDestinationTests.setUp(self)

    def run_copy(self, text):
        scripts = self.root / "tree" / "scripts" / "ios"
        shutil.rmtree(scripts.parent.parent, ignore_errors=True)
        scripts.mkdir(parents=True)
        for name in ("profile.py", "lib.sh", "Info.plist"):
            shutil.copy(HERE / name, scripts / name)
        (scripts / "provision.sh").write_text(text)
        syntax_ok(self, scripts / "provision.sh")
        self.log.unlink(missing_ok=True)
        (self.profiles / "created.mobileprovision").unlink(missing_ok=True)
        result = subprocess.run(
            ["bash", str(scripts / "provision.sh"), "--team", provision_tests.TEAM,
             "--bundle-id", provision_tests.BUNDLE, "--device", provision_tests.MAC,
             "--destination", provision_tests.MAC_DESTINATION, "--register-device"],
            env=self.env, text=True, capture_output=True, check=False)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
        return result, any(provision_tests.REGISTER in argv for argv in calls)

    def test_register_device_gate(self):
        text = (HERE / "provision.sh").read_text()
        mutant_text = without_gate(text, "register-device")
        result, registered = self.run_copy(text)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(registered, "the positive control did not pass the registration flag")
        result, registered = self.run_copy(mutant_text)
        self.assertFalse(registered)
        self.assertIn("unknown argument: --register-device", result.stderr)

    def test_provision_sh_has_only_the_register_device_gate(self):
        text = (HERE / "provision.sh").read_text()
        self.assertEqual(re.findall(r"# gate:([a-z-]+) (?:begin|end)", text),
                         ["register-device", "register-device"])


if __name__ == "__main__":
    unittest.main()
