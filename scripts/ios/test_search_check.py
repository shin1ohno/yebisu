"""Unit tests for the pure parts of search-check.py (#138): step times, record windows,
the peer log split by marker letters, and each step's verdict."""
import importlib.util
from pathlib import Path
import unittest

_spec = importlib.util.spec_from_file_location('search_check', Path(__file__).with_name('search-check.py'))
check = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(check)


def text(value, pane='w1:p1'):
    return {'ClientShellPaneInput': {'pane_id': pane, 'events': [{'TextCommit': value}]}}


def state(time, **fields):
    record = {'kind': 'search-state', 'time': time, 'open': False, 'phase': None, 'filter': ''}
    sdk = fields.pop('sdk', None)
    record.update(fields)
    if sdk is not None:
        record['sdk'] = sdk
    return record


UP = {'field_first_responder': True, 'gpui_text_first_responder': False,
      'gpui_drawing_first_responder': False, 'presentation_style': 7, 'field_text': ''}


class StepTimes(unittest.TestCase):
    def test_reads_only_well_formed_step_lines(self):
        stdout = 'noise\nSEARCH_STEP T-S1 10.5\nSEARCH_STEP T-S2 nope\nSEARCH_STEP T-S2 12\n'
        self.assertEqual(check.step_times(stdout), [('T-S1', 10.5), ('T-S2', 12.0)])

    def test_records_split_at_each_step_start(self):
        records = [state(9), state(10.5), state(11.9), state(12), {'kind': 'launch', 'time': 11}]
        windows = check.by_step(records, [('T-S1', 10.5), ('T-S2', 12.0)])
        self.assertEqual([r['time'] for r in windows['T-S1']], [10.5, 11.9])
        self.assertEqual([r['time'] for r in windows['T-S2']], [12])


class PeerSplit(unittest.TestCase):
    def test_inputs_before_each_marker_in_order(self):
        records = [text('a'), text('z'), text('w'), text('q', 'w2:p2'), text('x', 'w2:p2'), text('r')]
        flat, steps = check.split_peer(records, {'T-S3': 'z', 'T-S4': 'w', 'T-S6': 'x'})
        self.assertEqual(steps['T-S3'], [('text', 'a', 'w1:p1')])
        self.assertEqual(steps['T-S4'], [])
        self.assertEqual(steps['T-S6'], [('text', 'q', 'w2:p2')])
        self.assertEqual(steps['rest'], [('text', 'r', 'w1:p1')])
        self.assertEqual(check.count_text(flat, 'z'), 1)

    def test_ignores_records_that_are_not_pane_input(self):
        self.assertEqual(check.peer_events({'ClientShellEndpointRequest': {'request': '{}'}}), [])
        self.assertEqual(check.peer_events(['not', 'a', 'record']), [])
        paste = {'ClientShellPaneInput': {'pane_id': 'p', 'events': [{'Paste': 'word'}]}}
        self.assertEqual(check.peer_events(paste), [('paste', 'word', 'p')])


class Verdicts(unittest.TestCase):
    def test_up_needs_the_field_and_neither_gpui_view(self):
        self.assertTrue(check.up(state(1, open=True, phase='Open', sdk=UP)))
        self.assertFalse(check.up(state(1, open=True, phase='Open', sdk={**UP, 'gpui_text_first_responder': True})))
        self.assertFalse(check.up(state(1, open=True, phase='Presenting', sdk=UP)))
        self.assertFalse(check.up(state(1, open=True, phase='Open')))

    def test_popover_and_sheet_styles(self):
        windows = {'T-S1': [state(1, open=True, phase='Open', sdk=UP)]}
        compact = {'T-S9': [state(2, open=True, phase='Open', sdk={**UP, 'presentation_style': 1})]}
        result = check.verdicts(windows, [], {}, compact, '')
        self.assertTrue(result['T-S1'][0])
        self.assertTrue(result['T-S9'][0])
        wrong = check.verdicts({'T-S1': [state(1, open=True, phase='Open', sdk={**UP, 'presentation_style': 1})]},
                               [], {}, {}, '')
        self.assertFalse(wrong['T-S1'][0])
        self.assertFalse(wrong['T-S9'][0], 'no compact records is a failure, not a pass')

    def test_query_step_fails_when_the_peer_received_input(self):
        windows = {'T-S2': [state(1, open=True, phase='Open', filter='pro', sdk={**UP, 'field_text': 'pro'})]}
        clean = check.verdicts(windows, [], {'T-S3': []}, {}, '')
        leaked = check.verdicts(windows, [('text', 'p', 'w1:p1')], {'T-S3': [('text', 'p', 'w1:p1')]}, {}, '')
        self.assertTrue(clean['T-S2'][0])
        self.assertFalse(leaked['T-S2'][0])

    def test_marker_must_arrive_exactly_once(self):
        windows = {'T-S3': [state(1, open=False, filter='pro')]}
        stdout = 'SEARCH banner exists=true\n'
        once = check.verdicts(windows, [('text', 'z', 'w1:p1')], {}, {}, stdout)
        twice = check.verdicts(windows, [('text', 'z', 'w1:p1')] * 2, {}, {}, stdout)
        no_banner = check.verdicts(windows, [('text', 'z', 'w1:p1')], {}, {}, '')
        self.assertTrue(once['T-S3'][0])
        self.assertFalse(twice['T-S3'][0])
        self.assertFalse(no_banner['T-S3'][0])

    def test_every_step_has_a_verdict(self):
        result = check.verdicts({}, [], {}, {}, '')
        self.assertEqual(set(result), set(check.STEPS) | {'T-S9'})
        self.assertFalse(any(ok for ok, _ in result.values()), 'no records must never pass')


if __name__ == '__main__':
    unittest.main()
