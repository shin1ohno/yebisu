"""ci.yml's pull request filter for the edit-menu job, and the job's unchanged shape."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
WORKFLOW = (HERE.parents[1] / ".github/workflows/ci.yml").read_text()


class JobTests(unittest.TestCase):
    def job(self):
        match = re.search(r"^  edit-menu:\n(.*?)^  simulator-smoke:\n", WORKFLOW, re.S | re.M)
        self.assertIsNotNone(match)
        return match.group(1)

    def test_one_job_runs_the_four_checks_in_sequence_as_before(self):
        job = self.job()
        self.assertNotIn("matrix", job)
        self.assertIn("run: just guarded python3 scripts/ios/ci-sh1-152.py\n", job)
        self.assertIn("timeout-minutes: 90\n", job)
        self.assertIn("name: edit-menu-evidence\n", job)
        for evidence in ("ios-sh1-152", "ios-sh1-153", "ios-p8-native", "ios-selection-settings", "ios-sh1-156"):
            self.assertIn(f"target/{evidence}/", job)

    def test_no_retry_or_continue_on_error(self):
        job = self.job()
        self.assertNotIn("retry", job.lower())
        self.assertNotIn("continue-on-error", job)

    def test_the_driver_runs_the_four_scripts_in_order(self):
        driver = (HERE / "ci-sh1-152.py").read_text()
        names = re.findall(r'"([a-z0-9_-]+-check\.py)"', driver)
        self.assertEqual(names, ["sh1-152-check.py", "sh1-153-check.py", "p8-native-check.py", "selection-settings-check.py",
                                 "sh1-156-check.py"])
        # L61's secondary-click targets run on the same Simulator, right after SH1-152.
        self.assertIn('["sh1-152-check.py", "sh1_152_targets.py", "sh1-153-check.py"', driver)


def filter_script():
    """The shell body of the changes job's filter step, exactly as the runner executes it."""
    lines = WORKFLOW.splitlines()
    start = next(i for i, line in enumerate(lines) if line == "        run: |" and "set -euo pipefail" in lines[i + 1]) + 1
    body = []
    for line in lines[start:]:
        if line.strip() and not line.startswith(" " * 10):
            break
        body.append(line[10:])
    return "\n".join(body) + "\n"


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", *args],
                          check=True, capture_output=True, text=True).stdout.strip()


def decisions(event, changed, tmp, *, skipped_jobs="", invalid_diff=False):
    """Runs the real filter step in a throwaway repository and returns the boolean outputs."""
    repo = Path(tmp) / "repo"
    repo.mkdir()
    git(repo, "init", "-q", "-b", "main")
    (repo / "README.md").write_text("base\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "base")
    base = git(repo, "rev-parse", "HEAD")
    git(repo, "checkout", "-q", "-b", "topic")
    for path in changed:
        (repo / path).parent.mkdir(parents=True, exist_ok=True)
        (repo / path).write_text("changed\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "change")
    head = git(repo, "rev-parse", "HEAD")
    script = Path(tmp) / "filter.sh"
    script.write_text(filter_script())
    output = Path(tmp) / "output"
    output.write_text("")
    env = {
        **os.environ, "EVENT_NAME": event, "BASE_SHA": "missing" if invalid_diff else base,
        "HEAD_SHA": head, "RUNNER_TEMP": str(tmp), "GITHUB_OUTPUT": str(output),
        "SKIPPED_JOBS": skipped_jobs,
    }
    result = subprocess.run(["bash", str(script)], cwd=repo, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return {k: v == "true" for k, v in (line.split("=") for line in output.read_text().split())}


def menu_pattern():
    """The menu= pattern exactly as ci.yml defines it."""
    return re.search(r"^          menu='(.*)'$", WORKFLOW, re.M).group(1)


def menu_names_and_dirs():
    """The src/*.rs module names and the src/<dir>/ directories the pattern lists."""
    pattern = menu_pattern()
    names = re.search(r"src/\(\((.*?)\|edit_menu", pattern).group(1).split("|")
    dirs = re.search(r"\|\(([a-z0-9_|]+)\)/\)\)\)$", pattern).group(1).split("|")
    return names, dirs


# Modules main.rs declares that no edit-menu check drives. Evidence per entry: "unreferenced" = no other source file names it
# (git grep); "outside the checks" = referenced only from app.rs/terminal_view.rs paths that none of the four checks' YEBISU_*
# variables switch on (the review of PR 64 judged these by reading the callers); the rest say what the module is.
UNRELATED = {
    "action_view": "unreferenced",
    "app_setup": "unreferenced",
    "diagnostics": "outside the checks: diagnostics sheet and log export, called from app.rs",
    "error": "crate error type; a break here fails the compile in the check job",
    "find": "P13 search; test_find_search_change_skips_it keeps search changes off edit-menu pull requests. Its closing-press "
            "check runs on every terminal KeyDown (terminal_controls.rs) but owns no key unless search was open; push to main runs edit-menu",
    "find_view": "P13 search bar and its open/close focus, which no edit-menu check opens (same decision as find)",
    "front_layer": "outside the checks: P12 front layer, called from app.rs",
    "handoff_qa": "YEBISU_HANDOFF_QA is set only by testSH154CoreImage (SmokeQA.swift:532), which none of the four checks runs",
    "host_settings_availability": "referenced only by p15_settings_view and host_settings tests",
    "hosts": "outside the checks: host list, called from terminal_view.rs",
    "keep_awake": "display idle timer synced by app.rs and reconnect.rs; the edit-menu checks assert selection, "
                  "editing and input behavior without testing display sleep",
    "keys": "outside the checks: device keys, called from settings/key_ops.rs",
    "native_menus_qa": "P12 native menu QA scene (YEBISU_P12_QA), called from app.rs",
    "notification_settings": "unreferenced",
    "notification_view": "outside the checks: notification banner, called from app.rs",
    "notifications": "delivery and sound; only notifications/preferences.rs is matched",
    "p11_observer": "outside the checks: P11 responder observer, called from app.rs and terminal_controls.rs callbacks",
    "p15_settings_view": "unreferenced",
    "reconnect": "outside the checks: reconnect policy, called from terminal_view.rs",
    "settings_spike": "referenced only by settings_pages/INTEGRATION.md",
    "setup": "outside the checks: pairing setup, called from app.rs",
    "terminal_links": "L66 Command + press on a link; it returns at once unless Command is held, which no edit-menu "
                      "check does. Its hook is in terminal_view.rs, which the pattern matches",
    "upstream": "referenced only by settings_pages/INTEGRATION.md",
}


class PathFilterTests(unittest.TestCase):
    """Runs the changes job's own filter script, so the pattern and the push-to-main rule are both exercised."""

    def edit_menu(self, *paths, event="pull_request"):
        with tempfile.TemporaryDirectory() as tmp:
            return decisions(event, paths, tmp)["edit-menu"]

    def test_edit_menu_rs_change_runs_it(self):
        self.assertTrue(self.edit_menu("crates/yebisu/src/edit_menu.rs"))

    def test_find_search_change_skips_it(self):
        self.assertFalse(self.edit_menu("crates/yebisu/src/find/search.rs"))
        self.assertFalse(self.edit_menu("crates/yebisu/src/find.rs", "crates/yebisu/src/find/ui.rs", "crates/yebisu/src/find/tests.rs"))

    def test_other_app_code_skips_it(self):
        self.assertFalse(self.edit_menu("crates/yebisu/src/notifications.rs", "crates/yebisu/src/notifications/sound.rs", "crates/yebisu/src/hosts.rs",
                                        "crates/yebisu/src/host_settings/store.rs", "crates/yebisu/src/native_menus_qa.rs", "crates/yebisu/src/p11_observer.rs",
                                        "crates/yebisu/src/reconnect.rs", "crates/yebisu/src/upstream/mod.rs", "crates/yebisu/src/setup.rs",
                                        "crates/yebisu/locales/en/main.ftl", "crates/yebisu/src/handoff_cjk_tests.rs"))

    def test_keep_awake_only_changes_skip_it(self):
        for path in ("crates/yebisu/src/keep_awake.rs", "crates/yebisu/src/keep_awake/tests.rs"):
            with self.subTest(path=path):
                self.assertFalse(self.edit_menu(path))

    def test_push_to_main_and_manual_runs_always_run_it(self):
        self.assertTrue(self.edit_menu("crates/yebisu/src/find/search.rs", event="push"))
        self.assertTrue(self.edit_menu("docs/ci-simulator.md", event="push"))
        self.assertTrue(self.edit_menu("docs/ci-simulator.md", event="workflow_dispatch"))

    def test_docs_only_and_p12_only_pull_requests_skip(self):
        self.assertFalse(self.edit_menu("docs/ci-simulator.md", "README.md", "TODO.md"))
        self.assertFalse(self.edit_menu("scripts/ios/p12-qa/Tests/P12QA.swift", "scripts/ios/p12-qa/project.yml", "scripts/ios/p12-ci.py",
                                        "scripts/ios/p12-check.py", "scripts/ios/p12-oracle.py", "scripts/ios/test_p12_oracle.py",
                                        "scripts/ios/p12-mutations.py", "docs/p12-keyless-menus.md"))
        self.assertFalse(self.edit_menu("scripts/ios/sim.sh", "scripts/ios/smoke-check.py", "scripts/ios/p9-check.py", "scripts/ios/test_ci_sh1_152.py",
                                        "scripts/ios/test_sh1_152_check.py", "scripts/release/check-commit-messages.py", ".github/workflows/testflight.yml",
                                        "scripts/ios/fixtures/settings-writer-default.json"))

    def test_every_file_the_checks_exercise_runs_it(self):
        for path in (
            "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", "assets/fonts/SauceCodePro/LICENSE.txt",
            "justfile", "scripts/guarded.py", ".github/workflows/ci.yml",
            "scripts/ios/bundle.sh", "scripts/ios/Info.plist", "scripts/ios/lib.sh", "scripts/ios/ci-smoke.py", "scripts/ios/ci-sh1-152.py",
            "scripts/ios/sh1-152-check.py", "scripts/ios/sh1-153-check.py", "scripts/ios/p8-native-check.py", "scripts/ios/selection-settings-check.py",
            "scripts/ios/sh1_152_oracle.py", "scripts/ios/sh1_152_scenes.py", "scripts/ios/sh1_152_transitions.py",
            "scripts/ios/smoke-qa/Tests/EditMenuQA.swift", "scripts/ios/smoke-qa/Tests/SmokeQA.swift", "scripts/ios/smoke-qa/project.yml",
            "crates/yebisu/Cargo.toml", "crates/yebisu/src/main.rs", "crates/yebisu/src/app.rs",
            "crates/yebisu/src/edit_menu.rs", "crates/yebisu/src/edit_menu_gestures.rs", "crates/yebisu/src/edit_menu_qa.rs",
            "crates/yebisu/src/edit_menu_qa_sh1_152.rs", "crates/yebisu/src/edit_menu_tests.rs",
            "crates/yebisu/src/native_menus.rs", "crates/yebisu/src/native_edit_qa.rs", "crates/yebisu/src/native_observer_qa.rs",
            "crates/yebisu/src/terminal_controls.rs", "crates/yebisu/src/terminal_input.rs", "crates/yebisu/src/terminal_view.rs",
            "crates/yebisu/src/input.rs", "crates/yebisu/src/input_geometry.rs",
            "crates/yebisu/src/mock_peer.rs", "crates/yebisu/src/core_mock.rs", "crates/yebisu/src/core_view.rs", "crates/yebisu/src/core_paste_view.rs",
            "crates/yebisu/src/observation.rs", "crates/yebisu/src/observation/tests.rs",
            "crates/yebisu/src/handoff.rs", "crates/yebisu/src/handoff_view.rs",
            "crates/yebisu/src/settings.rs", "crates/yebisu/src/settings_view.rs", "crates/yebisu/src/settings/store.rs",
            "crates/yebisu/src/settings_sheet.rs", "crates/yebisu/src/settings_sheet/view.rs",
        ):
            with self.subTest(path=path):
                self.assertTrue(self.edit_menu(path))

    def test_every_path_added_after_review_runs_it(self):
        """Each path the first review found the checks driving but the pattern missing."""
        for path in (
            "crates/yebisu/fixtures/endpoint-snapshot-v1.json", "crates/yebisu/fixtures/endpoint-welcome-v1.json",
            "crates/yebisu/src/handoff_clipboard.rs", "crates/yebisu/src/handoff_ticket.rs",
            "crates/yebisu/src/core.rs", "crates/yebisu/src/core/paste.rs", "crates/yebisu/src/core/paste_path.rs", "crates/yebisu/src/core/worker.rs",
            "crates/yebisu/src/core/screen.rs", "crates/yebisu/src/core/store.rs", "crates/yebisu/src/core/publication_probe/peer.rs",
            "crates/yebisu/src/core_mock/paste.rs", "crates/yebisu/src/core_mock/transport.rs",
            "crates/yebisu/src/live.rs", "crates/yebisu/src/live/tests.rs",
            "crates/yebisu/src/notifications/preferences.rs", "crates/yebisu/src/theme_picker/mod.rs", "crates/yebisu/src/theme_picker/tests.rs",
            "crates/yebisu/src/animation_qa.rs", "crates/yebisu/src/animation_qa/tests.rs",
            "crates/yebisu/src/actions.rs", "crates/yebisu/src/settings_pages/fonts.rs", "crates/yebisu/src/settings_pages/keyboard.rs",
            "crates/yebisu/src/mock_commands.rs", "crates/yebisu/src/mock_notifications.rs",
            "crates/yebisu/src/link.rs", "crates/yebisu/src/theme.rs", "crates/yebisu/src/board.rs", "crates/yebisu/src/i18n.rs",
            "scripts/ios/sh1-156-check.py", "crates/yebisu/src/sh1_156_qa.rs",
            "crates/yebisu/src/input_retirement.rs", "crates/yebisu/src/native_retirement.rs",
            "scripts/ios/search-check.py", "scripts/ios/test_search_check.py",
        ):
            with self.subTest(path=path):
                self.assertTrue(self.edit_menu(path))

    def test_the_qa_layer_runs_it(self):
        """UI test redesign S4: all five checks launch with YEBISU_QA_AX, press QA identifiers and wait on qa.state."""
        for name in ("qa_ax", "qa_ax_core", "qa_ax_ids", "qa_ax_keys", "qa_ax_layer", "qa_ax_native", "qa_ax_peer", "qa_ax_root"):
            with self.subTest(module=name):
                self.assertTrue(self.edit_menu(f"crates/yebisu/src/{name}.rs"))

    def test_a_sibling_of_an_added_path_that_no_check_drives_still_skips(self):
        """The additions are exact: notifications/ is not taken whole, only preferences.rs."""
        self.assertFalse(self.edit_menu("crates/yebisu/src/notifications/sound.rs"))
        self.assertFalse(self.edit_menu("crates/yebisu/src/notifications/tests.rs"))
        self.assertFalse(self.edit_menu("crates/yebisu/fixtures-other/x.json"))

    def test_every_listed_app_file_exists(self):
        """A renamed or removed file would silently stop selecting the jobs."""
        root = HERE.parents[1]
        names, dirs = menu_names_and_dirs()
        self.assertIn("settings_view", names)
        self.assertIn("handoff_clipboard", names)
        self.assertIn("core", dirs)
        for name in names:
            with self.subTest(name=name):
                self.assertTrue((root / "crates/yebisu/src" / f"{name}.rs").is_file())
        for name in dirs:
            with self.subTest(directory=name):
                self.assertTrue((root / "crates/yebisu/src" / name).is_dir())
        self.assertTrue((root / "crates/yebisu/src/notifications/preferences.rs").is_file())
        self.assertTrue((root / "crates/yebisu/fixtures").is_dir())

    def test_every_module_in_main_is_matched_or_reviewed_as_unrelated(self):
        """The pattern is a list, so a module added later is silently left out. Fail until someone decides."""
        pattern = re.compile(menu_pattern())
        declared = set(re.findall(r"^mod ([a-z0-9_]+);", (HERE.parents[1] / "crates/yebisu/src/main.rs").read_text(), re.M))
        unclassified = []
        for name in sorted(declared):
            matched = any(pattern.search(f"crates/yebisu/src/{rel}") for rel in (f"{name}.rs", f"{name}/mod.rs"))
            if not matched and name not in UNRELATED and not name.endswith("_tests"):
                unclassified.append(name)
        self.assertEqual(unclassified, [], "main.rs declares modules neither matched by menu= in ci.yml nor listed in UNRELATED: add them to the "
                         "pattern if an edit-menu check drives them, else to UNRELATED with the reason")
        for name, reason in UNRELATED.items():
            with self.subTest(unrelated=name):
                self.assertIn(name, declared, f"{name} is no longer declared in main.rs; drop it from UNRELATED")
                self.assertFalse(any(pattern.search(f"crates/yebisu/src/{rel}") for rel in (f"{name}.rs", f"{name}/mod.rs")),
                                 f"{name} is now matched by the pattern; drop it from UNRELATED ({reason})")

    def test_unrelated_jobs_keep_their_own_filters(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(decisions("pull_request", ["crates/yebisu/src/find/search.rs"], tmp),
                             {"check": True, "simulator-smoke": True, "edit-menu": False, "layout-smoke": False, "layout-mutations": False})


if __name__ == "__main__":
    unittest.main()
