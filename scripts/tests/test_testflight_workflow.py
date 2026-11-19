"""Pin the credential-bearing TestFlight job: triggers, ref checks and step order."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/testflight.yml'
# Commands that run third-party or build code (build.rs, Homebrew formulae).
THIRD_PARTY = re.compile(r'\b(brew|cargo|rustup|guarded|distribution\.py build)\b')


def top_level_block(text, key):
    match = re.search(rf'^{key}:\n((?:[ \t]+.*\n|\n)*)', text, re.MULTILINE)
    return match.group(1) if match else ''


def steps(text):
    """Split the job's steps on their `      - ` list markers (no YAML dependency)."""
    body = re.sub(r'^\s*#.*\n', '', text.split('\n    steps:\n', 1)[1], flags=re.MULTILINE)
    parts = re.split(r'^      - ', body, flags=re.MULTILINE)[1:]
    return [{'text': part, 'name': part.split('\n', 1)[0]} for part in parts]


def index(found, predicate, what):
    matches = [position for position, step in enumerate(found) if predicate(step['text'])]
    if len(matches) != 1:
        raise AssertionError(f'expected exactly one {what} step, found {len(matches)}')
    return matches[0]


class TestFlightWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = WORKFLOW.read_text()
        self.steps = steps(self.workflow)
        self.assertGreater(len(self.steps), 8, 'positive control: steps were parsed')

    def test_distribution_actions_use_full_commit_pins(self):
        actions = re.findall(r'^\s*- uses:\s+(\S+)', self.workflow, re.MULTILINE)
        self.assertTrue(actions, 'positive control: workflow action steps were found')
        self.assertTrue(any(action.startswith('actions/checkout@') for action in actions))
        for action in actions:
            with self.subTest(action=action):
                self.assertRegex(action, r'^[^@]+@[0-9a-f]{40}$')
        self.assertIn('34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1', self.workflow)

    def test_only_manual_runs_on_main_reach_the_job(self):
        triggers = top_level_block(self.workflow, 'on')
        self.assertIn('workflow_dispatch', triggers, 'positive control: the on block was found')
        self.assertEqual(re.findall(r'^  ([a-z_]+):', triggers, re.MULTILINE), ['workflow_dispatch'])
        self.assertNotIn('tags', triggers)
        condition = re.search(r"^    if: >-\n((?:      .*\n)+)", self.workflow, re.MULTILINE).group(1)
        self.assertIn("github.event_name == 'workflow_dispatch'", condition)
        self.assertIn("github.ref == 'refs/heads/main'", condition)
        self.assertNotIn('refs/tags', condition)
        self.assertNotIn('||', condition)
        self.assertNotIn('refs/tags', self.workflow)

    def test_head_must_equal_the_origin_main_tip_before_secrets(self):
        check = index(self.steps, lambda text: 'git rev-parse HEAD' in text, 'origin/main tip')
        text = self.steps[check]['text']
        self.assertIn('git fetch --no-tags origin +refs/heads/main:refs/remotes/origin/main', text)
        self.assertIn('tip=$(git rev-parse refs/remotes/origin/main)', text)
        self.assertIn('if [ "$head" != "$tip" ]; then', text)
        self.assertIn('exit 1', text)
        self.assertNotIn('merge-base --is-ancestor', self.workflow)
        first_secret = min(position for position, step in enumerate(self.steps) if 'secrets.' in step['text'])
        self.assertLess(check, first_secret)

    def test_validate_precedes_third_party_code_and_files_follow_the_build(self):
        validate = index(self.steps, lambda text: 'testflight-secrets.py validate' in text, 'validate')
        materialize = index(self.steps, lambda text: 'testflight-secrets.py materialize' in text, 'materialize')
        build = index(self.steps, lambda text: 'distribution.py build' in text, 'build')
        package = index(self.steps, lambda text: 'distribution.py package' in text, 'package')
        upload = index(self.steps, lambda text: 'distribution.py upload' in text, 'upload')
        third_party = [position for position, step in enumerate(self.steps) if THIRD_PARTY.search(step['text'])]
        self.assertGreaterEqual(len(third_party), 4, 'positive control: rustup, brew, cargo and build found')
        self.assertLess(validate, min(third_party))
        self.assertLess(build, materialize)
        self.assertLess(materialize, package)
        self.assertLess(package, upload)
        self.assertEqual(len(re.findall(r'testflight-secrets\.py', self.workflow)), 2)

    def test_no_secret_reaches_a_step_that_runs_build_or_third_party_code(self):
        for step in self.steps:
            if THIRD_PARTY.search(step['text']):
                with self.subTest(step=step['name']):
                    self.assertNotIn('secrets.', step['text'])

    def test_materialize_receives_only_the_file_inputs(self):
        materialize = self.steps[index(self.steps, lambda text: 'materialize' in text, 'materialize')]['text']
        self.assertEqual(sorted(re.findall(r'secrets\.([A-Z0-9_]+)', materialize)),
                         ['ASC_KEY_P8', 'IOS_P12_BASE64', 'IOS_PROFILE_BASE64'])

    def test_private_files_are_removed_always(self):
        last = self.steps[-1]['text']
        self.assertIn('if: always()', last)
        self.assertIn('rm -rf -- "$RUNNER_TEMP/yebisu-testflight"', last)


if __name__ == '__main__':
    unittest.main()
