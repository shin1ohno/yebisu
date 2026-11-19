"""ci.yml's layout-smoke job and its pull request filter, and the driver the job runs.

The filter follows edit-menu's rule (test_ci_sh1_152.py): it lists what the checks drive, and every module
main.rs declares must be matched or reviewed as unrelated, so a module added later forces a decision."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import re
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


menu = load("test_ci_sh1_152", "test_ci_sh1_152.py")  # decisions() runs the real filter step in a throwaway repository
driver = load("ci_layout_smoke", "ci-layout-smoke.py")

CHECKS = ["p2-check.py", "p3-check.py", "p10-check.py"]


class JobTests(unittest.TestCase):
    def job(self):
        match = re.search(r"^  layout-smoke:\n(.*?)(?=^  [a-z][a-z-]*:\n|\Z)", WORKFLOW, re.S | re.M)
        self.assertIsNotNone(match)
        return match.group(1)

    def test_one_job_runs_the_driver_within_sixty_minutes(self):
        job = self.job()
        self.assertIn("runs-on: macos-26\n", job)
        self.assertIn("timeout-minutes: 60\n", job)
        self.assertIn("if: needs.changes.outputs.layout-smoke == 'true'\n", job)
        self.assertIn("run: just guarded python3 scripts/ios/ci-layout-smoke.py\n", job)
        self.assertNotIn("matrix", job)

    def test_no_retry_or_continue_on_error(self):
        job = self.job()
        self.assertNotIn("retry", job.lower().replace("receiving-system", ""))
        self.assertNotIn("continue-on-error", job)

    def test_built_like_the_edit_menu_job(self):
        edit_menu = re.search(r"^  edit-menu:\n(.*?)^  simulator-smoke:\n", WORKFLOW, re.S | re.M).group(1)
        for step in ("python3 scripts/guarded.py rustup show active-toolchain", "brew install just xcodegen", "just guarded cargo fetch --locked"):
            with self.subTest(step=step):
                self.assertIn(step, edit_menu)
                self.assertIn(step, self.job())

    def test_evidence_of_every_check_and_the_durations_are_uploaded_even_after_a_failure(self):
        job = self.job()
        self.assertIn("if: always()\n", job)
        self.assertIn("name: layout-smoke-evidence\n", job)
        for path in ("target/ios-layout-smoke/", "target/ios-p2/", "target/ios-p3/", "target/ios-p10/"):
            self.assertIn(path, job)

    def test_the_job_is_selected_by_the_changes_job(self):
        self.assertIn("layout-smoke: ${{ steps.filter.outputs.layout-smoke }}\n", WORKFLOW)
        self.assertIn('decide layout-smoke "$layout"\n', WORKFLOW)

    def test_the_driver_runs_the_three_checks_in_order(self):
        self.assertEqual(driver.CHECKS, CHECKS)
        text = (HERE / "ci-layout-smoke.py").read_text()
        line = re.search(r"^CHECKS = \[(.*)\]$", text, re.M).group(1)
        self.assertEqual(re.findall(r'"([a-z0-9_-]+-check\.py)"', line), CHECKS)
        for name in CHECKS:
            self.assertTrue((HERE / name).is_file(), name)

    def test_known_red_is_empty_since_p3_passed_on_hosted(self):
        # Integration 2: p3 passed every stage in run 44214913068, so no check is known red.
        self.assertEqual(driver.KNOWN_RED, {})
        for name in driver.KNOWN_RED:
            self.assertIn(name, CHECKS)

    def results(self, p2=0, p3=1, p10=0):
        return [{"check": "p2-check.py", "exit": p2}, {"check": "p3-check.py", "exit": p3}, {"check": "p10-check.py", "exit": p10}]

    # The known-red mechanism, exercised with p3 listed as it was in Integration 1.
    P3_RED = {"p3-check.py": "landscape rotation drops the private-peer connection (aed154e)"}

    @mock.patch.dict(driver.KNOWN_RED, P3_RED)
    def test_a_known_red_failure_is_logged_and_does_not_fail_the_job(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(driver.verdict(self.results()), [])
        self.assertIn("layout-smoke KNOWN RED: p3-check.py", out.getvalue())

    def test_negative_other_failures_still_fail_the_job(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(driver.verdict(self.results(p3=1)), ["p3-check.py"])
            self.assertEqual(driver.verdict(self.results(p2=1, p3=0)), ["p2-check.py"])
            self.assertEqual(driver.verdict(self.results(p3=0, p10=65)), ["p10-check.py"])
            self.assertEqual(driver.verdict(self.results(p3=0)[:2]), ["a check did not run"])

    @mock.patch.dict(driver.KNOWN_RED, P3_RED)
    def test_negative_a_known_red_that_passes_fails_the_job_until_removed(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(driver.verdict(self.results(p3=0)), ["p3-check.py"])
        self.assertIn("remove it from KNOWN_RED", out.getvalue())

    def test_every_check_the_driver_runs_takes_its_device_from_the_environment(self):
        for name in CHECKS:
            with self.subTest(check=name):
                self.assertIn("HERDR_IOS_SIMULATOR", (HERE / name).read_text())


def layout_pattern():
    return re.search(r"^          layout='(.*)'$", WORKFLOW, re.M).group(1)


def layout_names_and_dirs():
    pattern = layout_pattern()
    names = re.search(r"src/\(\((.*?)\)\\\.rs\$", pattern).group(1).split("|")
    dirs = re.search(r"\|\(([a-z0-9_|]+)\)/\)\)\)$", pattern).group(1).split("|")
    return names, dirs


# Modules main.rs declares that none of p2, p3 and p10 drive. They set only YEBISU_P2_OBSERVE, YEBISU_P11_KEYS_QA and YEBISU_QA_AX (p2),
# YEBISU_TERMINAL_QA and YEBISU_SMOKE_OBSERVE (p3), YEBISU_SMOKE_OBSERVE and YEBISU_SETTINGS_QA (p10).
# "unreferenced" = no other source file names it (git grep); "scene" = a QA scene another job's variable switches on.
UNRELATED = {
    "action_view": "unreferenced",
    "animation_qa": "scene: app.rs samples it when YEBISU_CORE_QA is set, which none of the three sets; edit-menu and simulator-smoke run it",
    "core": "scene: Herdr 2 core session behind YEBISU_CORE_QA (core_view.rs connect_core); simulator-smoke runs it (P9)",
    "core_mock": "scene: the mock core of YEBISU_CORE_QA",
    "core_paste_view": "scene: core image paste, YEBISU_CORE_QA only",
    "core_view": "scene: core controls row, drawn only after connect_core (YEBISU_CORE_QA)",
    "diagnostics": "outside the checks: diagnostics sheet and log export, called from app.rs",
    "edit_menu_qa": "scene: YEBISU_EDIT_MENU_QA, set only by testSH1153EditMenu",
    "edit_menu_qa_native": "scene: native edit menu QA, edit-menu job",
    "edit_menu_qa_scenes": "scene: edit menu QA, edit-menu job",
    "edit_menu_qa_sh1_152": "scene: YEBISU_SH1_152_CASE, edit-menu job",
    "error": "crate error type; a break here fails the compile in the check job",
    "find": "P13 search; no check of the three opens it. Push to main runs layout-smoke",
    "find_view": "P13 search bar, which none of the three opens",
    "front_layer": "outside the checks: P12 front layer, called from app.rs",
    "handoff_clipboard": "Paste reads the clipboard here; none of the three taps Paste",
    "handoff_qa": "scene: YEBISU_HANDOFF_QA, set only by testSH154CoreImage",
    "handoff_ticket": "core paste ticket; none of the three pastes",
    "host_settings_availability": "referenced only by p15_settings_view and host_settings tests",
    "native_edit_qa": "scene: YEBISU_NATIVE_QA, set only by testP8NativeEditing and the P11 observer",
    "native_menus_qa": "scene: P12 native menu QA (YEBISU_P12_QA), called from app.rs",
    "native_observer_qa": "scene: YEBISU_NATIVE_QA, set only by testP8NativeEditing and the P11 observer",
    "notification_settings": "unreferenced",
    "notifications": "delivery and sound; only notifications/preferences.rs is matched (p10 saves the policy)",
    "p11_observer": "outside the checks: P11 responder observer",
    "p15_settings_view": "unreferenced",
    "settings_spike": "referenced only by settings_pages/INTEGRATION.md",
    "sh1_156_qa": "scene: YEBISU_SH1_156_QA, set only by testSH1156PointerClickAfterDiscard",
    "upstream": "referenced only by settings_pages/INTEGRATION.md",
}


class PathFilterTests(unittest.TestCase):
    """Runs the changes job's own filter script, so the pattern and the push-to-main rule are both exercised."""

    def layout(self, *paths, event="pull_request"):
        with tempfile.TemporaryDirectory() as tmp:
            return menu.decisions(event, paths, tmp)["layout-smoke"]

    def test_every_file_the_three_checks_drive_runs_it(self):
        for path in (
            "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", "assets/fonts/SauceCodePro/LICENSE.txt", "justfile",
            "scripts/guarded.py", ".github/workflows/ci.yml",
            "scripts/ios/bundle.sh", "scripts/ios/Info.plist", "scripts/ios/lib.sh", "scripts/ios/sim.sh", "scripts/ios/bundle-provenance.py",
            "scripts/ios/compiler_inputs.py", "scripts/ios/screenshot-color.py", "scripts/ios/live_jsonl.py", "scripts/ios/ci-smoke.py",
            "scripts/ios/ci-layout-smoke.py", "scripts/ios/p2-check.py", "scripts/ios/p3-check.py", "scripts/ios/p10-check.py",
            "scripts/ios/smoke-qa/project.yml", "scripts/ios/smoke-qa/Tests/SmokeQA.swift", "scripts/ios/smoke-qa/Tests/SmokeQA+Keys.swift",
            "scripts/ios/smoke-qa/Tests/SmokeQA+Terminal.swift", "scripts/ios/smoke-qa/Tests/SmokeQA+Settings.swift",
            "crates/yebisu/Cargo.toml", "crates/yebisu/fixtures/endpoint-snapshot-v1.json",
            "crates/yebisu/src/main.rs", "crates/yebisu/src/app.rs", "crates/yebisu/src/app_setup.rs", "crates/yebisu/src/keys.rs",
            "crates/yebisu/src/keys/store.rs", "crates/yebisu/src/setup.rs", "crates/yebisu/src/setup/job.rs",
            "crates/yebisu/src/terminal_view.rs", "crates/yebisu/src/terminal_controls.rs", "crates/yebisu/src/terminal_input.rs",
            "crates/yebisu/src/input.rs", "crates/yebisu/src/input_geometry.rs", "crates/yebisu/src/mock_peer.rs",
            "crates/yebisu/src/board.rs", "crates/yebisu/src/theme.rs", "crates/yebisu/src/theme_picker/mod.rs",
            "crates/yebisu/src/hosts.rs", "crates/yebisu/src/hosts/scan.rs", "crates/yebisu/src/reconnect.rs",
            "crates/yebisu/src/notification_view.rs", "crates/yebisu/src/notification_view/qa.rs",
            "crates/yebisu/src/notifications/preferences.rs",
            "crates/yebisu/src/settings.rs", "crates/yebisu/src/settings/store.rs", "crates/yebisu/src/settings_view.rs",
            "crates/yebisu/src/settings_sheet.rs", "crates/yebisu/src/settings_sheet/view.rs", "crates/yebisu/src/settings_pages/fonts.rs",
            "crates/yebisu/src/handoff.rs", "crates/yebisu/src/handoff_view.rs", "crates/yebisu/src/edit_menu.rs",
            "crates/yebisu/src/edit_menu_gestures.rs", "crates/yebisu/src/native_menus.rs", "crates/yebisu/src/actions.rs",
            "crates/yebisu/src/i18n.rs", "crates/yebisu/src/link.rs", "crates/yebisu/src/live.rs", "crates/yebisu/src/live/tests.rs",
            "crates/yebisu/src/observation.rs", "crates/yebisu/src/observation/tests.rs", "crates/yebisu/src/mock_commands.rs",
            "crates/yebisu/src/mock_notifications.rs", "crates/yebisu/src/input_retirement.rs", "crates/yebisu/src/native_retirement.rs",
            # S2: p2 sets YEBISU_QA_AX, so the QA accessibility layer is what p2 drives.
            "crates/yebisu/src/qa_ax.rs", "crates/yebisu/src/qa_ax_ids.rs", "crates/yebisu/src/qa_ax_keys.rs",
            "crates/yebisu/src/qa_ax_layer.rs", "crates/yebisu/src/qa_ax_native.rs", "crates/yebisu/src/qa_ax_root.rs",
        ):
            with self.subTest(path=path):
                self.assertTrue(self.layout(path))

    def test_scene_and_search_changes_skip_it(self):
        self.assertFalse(self.layout("crates/yebisu/src/find/search.rs", "crates/yebisu/src/find.rs", "crates/yebisu/src/find_view.rs"))
        self.assertFalse(self.layout("crates/yebisu/src/core_view.rs", "crates/yebisu/src/core/paste.rs", "crates/yebisu/src/core_mock/paste.rs",
                                     "crates/yebisu/src/handoff_qa.rs", "crates/yebisu/src/sh1_156_qa.rs", "crates/yebisu/src/edit_menu_qa.rs",
                                     "crates/yebisu/src/edit_menu_qa_sh1_152.rs", "crates/yebisu/src/native_edit_qa.rs",
                                     "crates/yebisu/src/native_observer_qa.rs", "crates/yebisu/src/animation_qa.rs"))
        self.assertFalse(self.layout("crates/yebisu/src/diagnostics.rs", "crates/yebisu/src/front_layer.rs", "crates/yebisu/src/p11_observer.rs",
                                     "crates/yebisu/src/upstream/mod.rs", "crates/yebisu/src/host_settings/store.rs",
                                     "crates/yebisu/src/settings_spike/settings.ftl", "crates/yebisu/src/handoff_cjk_tests.rs"))

    def test_every_ftl_file_i18n_embeds_runs_it(self):
        """i18n.rs include_str!s these into the binary, so a text change in one repaints what p2 and p10 read."""
        included = re.findall(r'include_str!\("([^"]+\.ftl)"\)', (ROOT / "crates/yebisu/src/i18n.rs").read_text())
        self.assertGreaterEqual(len(included), 4)
        for name in included:
            with self.subTest(ftl=name):
                self.assertTrue((ROOT / "crates/yebisu/src" / name).is_file())
                self.assertTrue(self.layout("crates/yebisu/src/" + name))
        self.assertTrue(self.layout("crates/yebisu/src/locales/ipad-settings-sheet.ftl"))
        self.assertTrue(self.layout("crates/yebisu/src/commands.ftl"))

    def test_sh1_157_shell_and_layout_changes_run_it(self):
        """The frame around the terminal, the Hosts sheet p2 drives and the geometry they use (Integration 1)."""
        for path in ("crates/yebisu/src/shell.rs", "crates/yebisu/src/shell/frame.rs", "crates/yebisu/src/shell/hosts_sheet/view.rs",
                     "crates/yebisu/src/layout.rs", "crates/yebisu/src/layout/height.rs"):
            with self.subTest(path=path):
                self.assertTrue(self.layout(path))

    def test_push_to_main_and_manual_runs_always_run_it(self):
        self.assertTrue(self.layout("crates/yebisu/src/find/search.rs", event="push"))
        self.assertTrue(self.layout("docs/ci-simulator.md", event="push"))
        self.assertTrue(self.layout("docs/ci-simulator.md", event="workflow_dispatch"))

    def test_docs_and_other_jobs_files_skip_it(self):
        self.assertFalse(self.layout("docs/ci-simulator.md", "README.md", "TODO.md"))
        self.assertFalse(self.layout("scripts/ios/p12-qa/Tests/P12QA.swift", "scripts/ios/p12-ci.py", "scripts/ios/p12-check.py",
                                     "scripts/ios/p12-mutations.py", "docs/p12-keyless-menus.md"))
        self.assertFalse(self.layout("scripts/ios/ci-sh1-152.py", "scripts/ios/sh1-152-check.py", "scripts/ios/sh1-153-check.py",
                                     "scripts/ios/p8-native-check.py", "scripts/ios/selection-settings-check.py", "scripts/ios/sh1-156-check.py",
                                     "scripts/ios/p9-check.py", "scripts/ios/smoke-check.py", "scripts/ios/test_ci_layout_smoke.py",
                                     "scripts/ios/smokeqa-labels-check.py", "scripts/ios/test_smokeqa_labels_check.py",
                                     "scripts/release/check-commit-messages.py", ".github/workflows/testflight.yml"))

    def test_a_sibling_of_a_listed_path_that_no_check_drives_still_skips(self):
        self.assertFalse(self.layout("crates/yebisu/src/notifications/sound.rs"))
        self.assertFalse(self.layout("crates/yebisu/src/notifications.rs"))
        self.assertFalse(self.layout("crates/yebisu/fixtures-other/x.json"))

    def test_every_listed_app_file_exists(self):
        """A renamed or removed file would silently stop selecting the job."""
        names, dirs = layout_names_and_dirs()
        self.assertIn("app_setup", names)
        self.assertIn("reconnect", names)
        self.assertIn("settings_pages", dirs)
        for name in names:
            with self.subTest(name=name):
                self.assertTrue((ROOT / "crates/yebisu/src" / f"{name}.rs").is_file())
        for name in dirs:
            with self.subTest(directory=name):
                self.assertTrue((ROOT / "crates/yebisu/src" / name).is_dir())
        self.assertTrue((ROOT / "crates/yebisu/src/notifications/preferences.rs").is_file())
        self.assertTrue((ROOT / "crates/yebisu/fixtures").is_dir())

    def test_every_script_the_pattern_names_exists(self):
        scripts = re.search(r"scripts/ios/\(([^)]*)\)", layout_pattern()).group(1).split("|")
        for name in scripts:
            name = name.replace("\\", "")
            with self.subTest(script=name):
                self.assertTrue((HERE / name).exists())

    def test_every_module_in_main_is_matched_or_reviewed_as_unrelated(self):
        """The pattern is a list, so a module added later is silently left out. Fail until someone decides."""
        pattern = re.compile(layout_pattern())
        declared = set(re.findall(r"^mod ([a-z0-9_]+);", (ROOT / "crates/yebisu/src/main.rs").read_text(), re.M))
        unclassified = []
        for name in sorted(declared):
            matched = any(pattern.search(f"crates/yebisu/src/{rel}") for rel in (f"{name}.rs", f"{name}/mod.rs"))
            if not matched and name not in UNRELATED and not name.endswith("_tests"):
                unclassified.append(name)
        self.assertEqual(unclassified, [], "main.rs declares modules neither matched by layout= in ci.yml nor listed in UNRELATED: add them to "
                         "the pattern if p2, p3 or p10 drive them, else to UNRELATED with the reason")
        for name, reason in UNRELATED.items():
            with self.subTest(unrelated=name):
                self.assertIn(name, declared, f"{name} is no longer declared in main.rs; drop it from UNRELATED")
                self.assertFalse(any(pattern.search(f"crates/yebisu/src/{rel}") for rel in (f"{name}.rs", f"{name}/mod.rs")),
                                 f"{name} is now matched by the pattern; drop it from UNRELATED ({reason})")

    def test_the_other_jobs_keep_their_own_filters(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(menu.decisions("pull_request", ["crates/yebisu/src/find/search.rs"], tmp),
                             {"check": True, "simulator-smoke": True, "edit-menu": False, "layout-smoke": False})
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(menu.decisions("pull_request", ["scripts/ios/p3-check.py"], tmp),
                             {"check": True, "simulator-smoke": False, "edit-menu": False, "layout-smoke": True})


class DriverTests(unittest.TestCase):
    """The driver with its subprocess calls replaced: order, durations, cleanup and the exit status."""

    def run_main(self, exits, boot_fails=False):
        calls = []
        evidence = tempfile.TemporaryDirectory()
        self.addCleanup(evidence.cleanup)
        queue = list(exits)

        def fake_run(command, **kwargs):
            calls.append(command)
            if command[:2] == ["bash", "-c"]:
                if boot_fails:
                    raise driver.subprocess.CalledProcessError(1, command)
                return mock.Mock(returncode=0)
            if command[:1] == [driver.sys.executable]:
                return mock.Mock(returncode=queue.pop(0))
            return mock.Mock(returncode=0)

        def fake_output(command, **kwargs):
            return json.dumps({"devices": {}}) if "list" in command else "FAKE-UDID\n"

        with mock.patch.object(driver.subprocess, "run", fake_run), mock.patch.object(driver.subprocess, "check_output", fake_output), \
                mock.patch.object(driver.smoke, "select_template", return_value=("iPad", "iOS-27-0")), \
                mock.patch.object(driver, "EVIDENCE", Path(evidence.name)):
            with contextlib.redirect_stdout(io.StringIO()):
                try:
                    driver.main()
                    code = 0
                except SystemExit as stop:
                    code = stop.code
                except driver.subprocess.CalledProcessError:
                    code = "error"
        durations = json.loads((Path(evidence.name) / "durations.json").read_text())
        return code, calls, durations

    def scripts_run(self, calls):
        return [Path(c[1]).name for c in calls if c[:1] == [driver.sys.executable]]

    @mock.patch.dict(driver.KNOWN_RED, {"p3-check.py": "known red for this test"})
    def test_all_three_checks_run_in_order_on_one_simulator_that_is_deleted(self):
        # With p3 known red, its failure alone leaves the job green.
        code, calls, durations = self.run_main([0, 1, 0])
        self.assertEqual(code, 0)
        self.assertEqual(self.scripts_run(calls), CHECKS)
        self.assertEqual(sum(1 for c in calls if c[:3] == ["xcrun", "simctl", "delete"]), 1)
        self.assertEqual([c["check"] for c in durations["checks"]], CHECKS)
        self.assertEqual([c["exit"] for c in durations["checks"]], [0, 1, 0])
        self.assertIn("total_seconds", durations)

    def test_a_failing_check_does_not_stop_the_others_but_fails_the_job(self):
        code, calls, durations = self.run_main([1, 1, 0])
        self.assertEqual(code, 1)
        self.assertEqual(self.scripts_run(calls), CHECKS)
        self.assertEqual([c["exit"] for c in durations["checks"]], [1, 1, 0])
        self.assertEqual(sum(1 for c in calls if c[:3] == ["xcrun", "simctl", "delete"]), 1)

    @mock.patch.dict(driver.KNOWN_RED, {"p3-check.py": "known red for this test"})
    def test_a_known_red_that_passes_fails_the_job(self):
        code, _, _ = self.run_main([0, 0, 0])
        self.assertEqual(code, 1)

    def test_a_boot_failure_still_deletes_the_simulator(self):
        code, calls, durations = self.run_main([], boot_fails=True)
        self.assertEqual(code, "error")
        self.assertEqual(self.scripts_run(calls), [])
        self.assertEqual(sum(1 for c in calls if c[:3] == ["xcrun", "simctl", "delete"]), 1)
        self.assertEqual(durations["checks"], [])


if __name__ == "__main__":
    unittest.main()
