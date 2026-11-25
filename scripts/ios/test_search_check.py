"""Unit tests for the pure parts of search-check.py (#138): step times, record windows, the
numbered events per step, the peer log split by marker letters, and each step's verdict."""
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


class StepEvents(unittest.TestCase):
    def test_each_event_lands_in_the_step_of_the_first_record_carrying_it(self):
        records = [
            state(10.6, events=[[1, 'Opened']]),
            # Dismissed and InputRestored in one frame: last_event shows only the second.
            state(11.0, events=[[1, 'Opened'], [2, 'Dismissed(Return)'], [3, 'InputRestored(Text)']],
                  last_event='InputRestored(Text)'),
            state(12.5, events=[[2, 'Dismissed(Return)'], [3, 'InputRestored(Text)'], [4, 'Opened']]),
            state(12.6, events=[['x', 'bad'], [5]]),
        ]
        events = check.step_events(records, [('T-S1', 10.5), ('T-S2', 12.0)])
        self.assertEqual([e for _, e in events['T-S1']], ['Opened', 'Dismissed(Return)', 'InputRestored(Text)'])
        self.assertEqual(events['T-S2'], [(12.5, 'Opened')])


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
        events = {'T-S3': [(1, 'Dismissed(Return)')]}
        stdout = 'SEARCH banner exists=true\n'
        once = check.verdicts(windows, [('text', 'z', 'w1:p1')], {}, {}, stdout, events)
        twice = check.verdicts(windows, [('text', 'z', 'w1:p1')] * 2, {}, {}, stdout, events)
        no_banner = check.verdicts(windows, [('text', 'z', 'w1:p1')], {}, {}, '', events)
        no_return = check.verdicts(windows, [('text', 'z', 'w1:p1')], {}, {}, stdout, {})
        self.assertTrue(once['T-S3'][0])
        self.assertFalse(twice['T-S3'][0])
        self.assertFalse(no_banner['T-S3'][0])
        self.assertFalse(no_return['T-S3'][0], 'closing must be a Dismissed(Return)')

    def test_cancel_and_outside_tap_both_needed_with_the_filter_kept(self):
        windows = {'T-S4': [state(1, open=True, phase='Open', filter='pro'), state(2, filter='pro')]}
        peer = [('text', 'w', 'w1:p1')]
        both = {'T-S4': [(1, 'Dismissed(Cancel)'), (2, 'Dismissed(OutsideTap)')]}
        self.assertTrue(check.verdicts(windows, peer, {'T-S4': []}, {}, '', both)['T-S4'][0])
        cancel_only = {'T-S4': [(1, 'Dismissed(Cancel)')]}
        self.assertFalse(check.verdicts(windows, peer, {'T-S4': []}, {}, '', cancel_only)['T-S4'][0])
        cleared = {'T-S4': windows['T-S4'] + [state(3, filter='')]}
        self.assertFalse(check.verdicts(cleared, peer, {'T-S4': []}, {}, '', both)['T-S4'][0],
                         'a tap that cleared the field is not a Cancel that keeps the filter')

    def test_paste_step_needs_clear_filter_to_empty_field_and_filter(self):
        pasted = state(2, open=True, phase='Open', filter='pro searchpasteword',
                       sdk={**UP, 'field_text': 'pro searchpasteword'})
        emptied = state(5, open=True, phase='Open', filter='', sdk=UP)
        stdout = 'SEARCH_MARK clear 4\n'
        events = {'T-S5': [(4.5, 'ClearRequested')]}
        self.assertTrue(check.verdicts({'T-S5': [pasted, emptied]}, [], {}, {}, stdout, events)['T-S5'][0])
        stale_field = state(5, open=True, phase='Open', filter='', sdk={**UP, 'field_text': 'pro searchpasteword'})
        self.assertFalse(check.verdicts({'T-S5': [pasted, stale_field]}, [], {}, {}, stdout, events)['T-S5'][0],
                         'the filter cleared but the field kept its text (no set_query)')
        self.assertFalse(check.verdicts({'T-S5': [pasted, emptied]}, [], {}, {}, stdout, {})['T-S5'][0])

    def test_background_step_reads_the_state_at_each_mark(self):
        # Records are written on change: the state at a mark is the last record before it.
        up = state(3, open=True, phase='Open', sdk={**UP, 'phase': 'Presented'})
        # The iPad hide key ends editing: the field is no longer first responder, the panel stays.
        hidden = state(6, open=True, phase='Open', sdk={**UP, 'phase': 'Presented', 'field_first_responder': False})
        stdout = 'SEARCH_MARK foreground 4\nSEARCH_MARK close 8\n'
        peer = [('text', 'u', 'w1:p1')]
        kept = {'T-S8': [(9, 'Dismissed(Cancel)')]}
        self.assertTrue(check.verdicts({'T-S8': [up, hidden]}, peer, {}, {}, stdout, kept)['T-S8'][0])
        lost = {'T-S8': [(6, 'Dismissed(OutsideTap)'), (9, 'Dismissed(Cancel)')]}
        self.assertFalse(check.verdicts({'T-S8': [up, hidden]}, peer, {}, {}, stdout, lost)['T-S8'][0])
        late = state(5, open=True, phase='Open', sdk={**UP, 'phase': 'Presented'})
        self.assertFalse(check.verdicts({'T-S8': [late, hidden]}, peer, {}, {}, stdout, kept)['T-S8'][0],
                         'first responder only after the foreground mark is not "kept"')
        gone = state(6, open=False, sdk={**UP, 'phase': 'None', 'field_first_responder': False})
        self.assertFalse(check.verdicts({'T-S8': [up, gone]}, peer, {}, {}, stdout, kept)['T-S8'][0],
                         'the hide key must leave the panel up')

    def test_every_step_has_a_verdict(self):
        result = check.verdicts({}, [], {}, {}, '')
        self.assertEqual(set(result), set(check.STEPS) | {'T-S9'})
        self.assertFalse(any(ok for ok, _ in result.values()), 'no records must never pass')


if __name__ == '__main__':
    unittest.main()
