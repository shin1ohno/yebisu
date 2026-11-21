"""Exercise the change selector and GitHub API evidence gate from ci.yml."""

import copy
import contextlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import unittest
from unittest.mock import patch

from test_ci_sh1_152 import WORKFLOW, decisions


MACOS_JOBS = ("check", "simulator-smoke", "edit-menu", "layout-smoke")
REPOSITORY = "shin1ohno/yebisu"
MAIN = "1" * 40
HEAD = "2" * 40
MERGE = "3" * 40
TREE = "4" * 40
OTHER_TREE = "5" * 40
RUN = 44703167170
RUN_LIST = f"actions/workflows/ci.yml/runs?head_sha={HEAD}&event=pull_request&per_page=100"
JOB_LIST = f"actions/runs/{RUN}/attempts/2/jobs?per_page=100"
LOG = "actions/jobs/3/logs"
CHECKOUT_STEP = f"Verify tested commit {MERGE}"


class WorkflowSelectionTests(unittest.TestCase):
    def test_edit_menu_verifies_the_checkout_named_in_its_step_metadata(self):
        job = WORKFLOW.split("  edit-menu:\n", 1)[1].split("  simulator-smoke:\n", 1)[0]
        self.assertIn(
            "      - name: Verify tested commit ${{ github.sha }}\n"
            "        env:\n"
            "          EXPECTED_SHA: ${{ github.sha }}\n"
            '        run: test "$(git rev-parse HEAD)" = "$EXPECTED_SHA"\n',
            job,
        )

    def test_nightly_requests_every_job_and_bypasses_reuse(self):
        self.assertRegex(WORKFLOW, r"(?m)^    - cron: '0 18 \* \* \*'")
        self.assertIn("if: github.event_name == 'pull_request' || github.event_name == 'schedule'", WORKFLOW)
        self.assertIn("range=HEAD^..HEAD", WORKFLOW)
        self.assertIn("if: github.event_name == 'push' && github.ref == 'refs/heads/main'", WORKFLOW)
        with tempfile.TemporaryDirectory() as tmp:
            selected = decisions("schedule", ["docs/ci-simulator.md"], tmp, skipped_jobs=" ".join(MACOS_JOBS))
        self.assertEqual(selected, dict.fromkeys((*MACOS_JOBS, "layout-mutations"), True))

    def test_main_push_omits_only_the_jobs_named_by_evidence(self):
        for skipped_jobs in ("", "simulator-smoke", "check edit-menu"):
            with self.subTest(skipped_jobs=skipped_jobs), tempfile.TemporaryDirectory() as tmp:
                selected = decisions("push", ["docs/ci-simulator.md"], tmp, skipped_jobs=skipped_jobs)
                expected = {name: name not in skipped_jobs.split() for name in MACOS_JOBS}
                self.assertEqual(selected, {**expected, "layout-mutations": True})
        self.assertIn("        continue-on-error: true\n        env:\n          GH_TOKEN:", WORKFLOW)
        self.assertIn("SKIPPED_JOBS: ${{ steps.reuse.outcome == 'success' && steps.reuse.outputs.jobs || '' }}", WORKFLOW)

    def test_mutations_step_and_upload_use_the_changes_output(self):
        condition = "needs.changes.outputs.layout-mutations == 'true'"
        self.assertIn("layout-mutations: ${{ steps.filter.outputs.layout-mutations }}", WORKFLOW)
        self.assertIn(f"- name: Layout geometry source mutations\n        if: {condition}", WORKFLOW)
        self.assertIn(f"- name: Upload layout mutation evidence\n        if: always() && {condition}", WORKFLOW)

    def test_pr_mutations_match_the_plan_paths(self):
        for path in (
            "crates/yebisu/src/layout.rs", "crates/yebisu/src/layout/height.rs",
            "crates/yebisu/src/shell_tests.rs", "crates/yebisu/src/settings/input_options.rs",
            "crates/yebisu/src/settings_pages/toolbar_sync.rs", "crates/yebisu/src/core/screen.rs",
            "crates/yebisu/src/core/screen_tests/panel.rs", "crates/yebisu/src/terminal_input.rs",
            "scripts/ios/layout-mutations.py", "scripts/guarded.py", ".github/workflows/ci.yml",
            "Cargo.lock", "Cargo.toml", "crates/yebisu/Cargo.toml", "rust-toolchain.toml",
            ".cargo/config.toml", "justfile",
        ):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as tmp:
                self.assertTrue(decisions("pull_request", [path], tmp)["layout-mutations"])
        unrelated = ["docs/ci-simulator.md", "crates/yebisu/src/core/screening.rs",
                     "crates/yebisu/src/settings_sheet.rs", "crates/yebisu/src/core/paste.rs",
                     "scripts/ios/p12-mutations.py", ".github/workflows/testflight.yml"]
        with tempfile.TemporaryDirectory() as tmp:
            self.assertFalse(decisions("pull_request", unrelated, tmp)["layout-mutations"])

    def test_missing_pr_diff_keeps_pr_jobs_without_claiming_mutations_matched(self):
        with tempfile.TemporaryDirectory() as tmp:
            selected = decisions("pull_request", ["docs/ci-simulator.md"], tmp, invalid_diff=True)
        self.assertEqual(selected, {"check": True, "simulator-smoke": True, "edit-menu": True,
                                    "layout-smoke": False, "layout-mutations": False})

    def test_test_only_paths_still_select_the_applicable_simulator_jobs(self):
        for path, edit_menu in (("crates/yebisu/src/find/settle_tests.rs", False),
                                ("crates/yebisu/src/layout/tests.rs", True),
                                ("scripts/ios/smoke-qa/Tests/SmokeQA.swift", True)):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as tmp:
                selected = decisions("pull_request", [path], tmp)
                self.assertTrue(selected["simulator-smoke"])
                self.assertEqual(selected["edit-menu"], edit_menu)


def successful_jobs():
    return [
        {
            "id": index, "name": name, "run_id": RUN, "head_sha": HEAD, "runner_id": 101,
            "status": "completed", "conclusion": "success",
            "steps": [{"name": step, "status": "completed", "conclusion": "success"} for step in (
                "Set up job",
                {"check": "Layout geometry source mutations", "edit-menu": CHECKOUT_STEP}.get(name, name),
                "Complete job",
            )],
        }
        for index, name in enumerate(MACOS_JOBS, 1)
    ]


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        code = re.search(r"python3 - <<'PYTHON'\n(.*?)^          PYTHON$", WORKFLOW, re.M | re.S)
        self.assertIsNotNone(code, "the main-tree proof step is missing")
        self.step = {"__name__": "test_ci_job_selection"}
        exec(compile(textwrap.dedent(code.group(1)), ".github/workflows/ci.yml", "exec"), self.step)
        self.jobs = successful_jobs()
        self.pr = {
            "number": 91, "base": {"ref": "main", "repo": {"full_name": REPOSITORY}},
            "head": {"sha": HEAD, "ref": "ci/cargo-cache", "repo": {"full_name": REPOSITORY}},
            "merge_commit_sha": MAIN, "merged_at": "2026-11-20T11:12:07Z",
        }
        self.run = {
            "id": RUN, "run_attempt": 2, "event": "pull_request", "head_sha": HEAD,
            "head_branch": "ci/cargo-cache", "head_repository": {"full_name": REPOSITORY},
            "created_at": "2026-11-20T08:31:05Z", "status": "completed", "pull_requests": [],
            "path": ".github/workflows/ci.yml",
        }
        self.responses = {
            **{f"git/commits/{sha}": {"sha": sha, "tree": {"sha": TREE}} for sha in (MAIN, HEAD, MERGE)},
            f"commits/{MAIN}/pulls?per_page=100": [self.pr],
            RUN_LIST: {"total_count": 1, "workflow_runs": [self.run]},
            JOB_LIST: {"total_count": len(self.jobs), "jobs": self.jobs},
            LOG: f"2026-11-20T10:28:25.3897710Z [command]/opt/homebrew/bin/git log -1 --format=%H\n"
                 f"2026-11-20T10:28:25.3986610Z {MERGE}\n",
        }
        self.requests = []

    def run_gh(self, args, **kwargs):
        command, subcommand, path = args
        self.assertEqual((command, subcommand), ("gh", "api"))
        self.assertTrue(kwargs["capture_output"])
        self.assertTrue(path.startswith(f"repos/{REPOSITORY}/"), path)
        key = path.removeprefix(f"repos/{REPOSITORY}/")
        self.requests.append(key)
        response = self.responses[key]
        if isinstance(response, Exception):
            raise response
        stdout = response if isinstance(response, str) else json.dumps(response)
        return subprocess.CompletedProcess(args, 0, stdout=stdout, stderr="")

    def run_step(self):
        with tempfile.TemporaryDirectory() as tmp:
            output, summary = Path(tmp) / "output", Path(tmp) / "summary"
            output.touch()
            summary.touch()
            env = {"GITHUB_REPOSITORY": REPOSITORY, "GITHUB_SHA": MAIN, "GITHUB_OUTPUT": str(output),
                   "GITHUB_STEP_SUMMARY": str(summary)}
            stdout = io.StringIO()
            with (
                patch.dict(os.environ, env),
                patch.object(subprocess, "run", side_effect=self.run_gh),
                contextlib.redirect_stdout(stdout),
            ):
                self.step["main"]()
            self.logs, self.summary = stdout.getvalue(), summary.read_text()
            value = output.read_text().strip()
        return value.removeprefix("jobs=").split()

    def test_exact_tree_and_complete_latest_attempt_give_per_job_receipts(self):
        self.assertEqual(self.run_step(), list(MACOS_JOBS))
        self.assertIn(TREE, self.summary)
        self.assertNotIn(LOG, self.requests)
        self.assertIn(f"[edit-menu](https://github.com/{REPOSITORY}/actions/runs/{RUN}/job/3)", self.summary)

    def test_one_skipped_step_is_enough_to_run_check_again(self):
        self.jobs[0]["steps"][1]["conclusion"] = "skipped"
        self.jobs[1]["conclusion"] = "failure"
        self.jobs[3]["steps"] = []
        self.jobs[3]["runner_id"] = 0
        self.assertEqual(self.run_step(), ["edit-menu"])

    def test_missing_mutation_step_and_absent_job_in_latest_attempt_run_again(self):
        del self.jobs[0]["steps"][1]
        self.jobs.pop(1)
        self.responses[JOB_LIST]["total_count"] -= 1
        self.assertEqual(self.run_step(), ["edit-menu", "layout-smoke"])

    def test_checkout_tree_mismatch_runs_edit_menu_again(self):
        self.responses[f"git/commits/{MERGE}"]["tree"]["sha"] = OTHER_TREE
        self.assertEqual(self.run_step(), ["check", "simulator-smoke", "layout-smoke"])

    def test_missing_or_ambiguous_checkout_proof_runs_edit_menu_again(self):
        steps = self.jobs[2]["steps"]
        for checkout_steps in (
            [],
            [{**steps[1], "name": "Verify tested commit bad-sha"}],
            [steps[1], dict(steps[1])],
            [{**steps[1], "conclusion": "failure"}],
        ):
            with self.subTest(checkout_steps=checkout_steps), patch.dict(
                self.jobs[2], {"steps": [steps[0], *checkout_steps, steps[-1]]}
            ):
                self.assertEqual(self.run_step(), ["check", "simulator-smoke", "layout-smoke"])

    def test_gh_denial_puts_the_status_and_message_in_the_log_and_summary(self):
        token = "example-workflow-token"
        error = subprocess.CalledProcessError(
            1, ["gh", "api"], output="response body must stay off the summary",
            stderr=f"gh: Resource not accessible by integration (HTTP 403)\ncredential: {token}\n",
        )
        with patch.dict(os.environ, {"GH_TOKEN": token}), patch.dict(self.responses, {JOB_LIST: error}):
            self.assertEqual(self.run_step(), [])
        for output in (self.logs, self.summary):
            self.assertIn("gh: Resource not accessible by integration (HTTP 403)", output)
            self.assertIn("running every macOS job", output)
            self.assertNotIn(token, output)
            self.assertNotIn("response body must stay off", output)

    def test_missing_conflicting_or_failed_api_evidence_runs_the_full_suite(self):
        cases = (
            (f"git/commits/{HEAD}", {"sha": HEAD, "tree": {"sha": OTHER_TREE}}, "trees differ"),
            (f"commits/{MAIN}/pulls?per_page=100", [self.pr, self.pr], "no unique associated PR"),
            (RUN_LIST, {"total_count": 2, "workflow_runs": [self.run, self.run]}, "no unique CI run"),
            (RUN_LIST, {"total_count": 0, "workflow_runs": []}, "no unique CI run"),
            (RUN_LIST, {"total_count": 1, "workflow_runs": [{**self.run, "status": "in_progress"}]},
             "does not identify a completed run"),
            (RUN_LIST, {"total_count": 1, "workflow_runs": [{**self.run, "path": None}]},
             "does not identify a completed run"),
            (JOB_LIST, {"total_count": 5, "jobs": self.jobs}, "incomplete job listing"),
            (JOB_LIST, subprocess.CalledProcessError(1, ["gh", "api"]), "running every macOS job"),
        )
        for key, response, log in cases:
            with self.subTest(key=key, log=log), patch.dict(self.responses, {key: response}):
                self.assertEqual(self.run_step(), [])
                self.assertIn(log, self.logs)

    def test_duplicate_job_is_ambiguous_even_when_both_pass(self):
        self.jobs.append(copy.deepcopy(self.jobs[0]))
        self.responses[JOB_LIST]["total_count"] += 1
        self.assertEqual(self.run_step(), ["simulator-smoke", "edit-menu", "layout-smoke"])


if __name__ == "__main__":
    unittest.main()
