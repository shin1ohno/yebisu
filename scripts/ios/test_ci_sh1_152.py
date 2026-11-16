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
        for evidence in ("ios-sh1-152", "ios-sh1-153", "ios-p8-native", "ios-selection-settings"):
            self.assertIn(f"target/{evidence}/", job)

    def test_no_retry_or_continue_on_error(self):
        job = self.job()
        self.assertNotIn("retry", job.lower())
        self.assertNotIn("continue-on-error", job)

    def test_the_driver_runs_the_four_scripts_in_order(self):
        driver = (HERE / "ci-sh1-152.py").read_text()
        names = re.findall(r'"([a-z0-9_-]+-check\.py)"', driver)
        self.assertEqual(names, ["sh1-152-check.py", "sh1-153-check.py", "p8-native-check.py", "selection-settings-check.py"])


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


def decisions(event, changed, tmp):
    """Runs the real filter step in a throwaway repository and returns {job: 'run'|'skip'}."""
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
    env = {**os.environ, "EVENT_NAME": event, "BASE_SHA": base, "HEAD_SHA": head, "RUNNER_TEMP": str(tmp), "GITHUB_OUTPUT": str(output)}
    result = subprocess.run(["bash", str(script)], cwd=repo, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return {k: v == "true" for k, v in (line.split("=") for line in output.read_text().split())}


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
        self.assertFalse(self.edit_menu("crates/yebisu/src/notifications.rs", "crates/yebisu/src/hosts.rs", "crates/yebisu/src/handoff_clipboard.rs",
                                        "crates/yebisu/src/native_menus_qa.rs", "crates/yebisu/src/board.rs"))

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

    def test_every_listed_app_file_exists(self):
        """A renamed or removed file would silently stop selecting the jobs."""
        root = HERE.parents[1]
        pattern = re.search(r"^          menu='(.*)'$", WORKFLOW, re.M).group(1)
        names = re.search(r"src/\(\((.*?)\|edit_menu", pattern).group(1).split("|")
        self.assertIn("settings_view", names)
        for name in names:
            with self.subTest(name=name):
                self.assertTrue((root / "crates/yebisu/src" / f"{name}.rs").is_file())

    def test_unrelated_jobs_keep_their_own_filters(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(decisions("pull_request", ["crates/yebisu/src/find/search.rs"], tmp),
                             {"check": True, "simulator-smoke": True, "edit-menu": False})


if __name__ == "__main__":
    unittest.main()
