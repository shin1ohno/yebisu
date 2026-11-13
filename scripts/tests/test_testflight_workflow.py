"""Keep actions in the credential-bearing distribution job immutable."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class TestFlightWorkflowTests(unittest.TestCase):
    def test_distribution_actions_use_full_commit_pins(self):
        workflow = (ROOT / '.github/workflows/testflight.yml').read_text()
        actions = re.findall(r'^\s*- uses:\s+(\S+)', workflow, re.MULTILINE)
        self.assertTrue(actions, 'positive control: workflow action steps were found')
        self.assertTrue(any(action.startswith('actions/checkout@') for action in actions))
        for action in actions:
            with self.subTest(action=action):
                self.assertRegex(action, r'^[^@]+@[0-9a-f]{40}$')
        self.assertIn('34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1', workflow)


if __name__ == '__main__':
    unittest.main()
