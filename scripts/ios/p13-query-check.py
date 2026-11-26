#!/usr/bin/env python3
"""Actual captured-client P13 query receipts; no physical IME/OS guarantee.

--run is exclusive-leased Mac only. Offline validation is a receipt check,
never evidence that the app or UIKit actually executed.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
# Test-support Cut/first-query helper (herdr-uikit PR 14), unchanged at the pinned commit.
SDK = '8339e132e004a05119e4fddf4fb637b997e4cdcb'
FIELD = 'crates/yebisu/src/find/field.rs'
QUERIES = 'crates/yebisu/src/find/native_queries.rs'
LENGTH = '''    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        if let Some(length) = self.forward(cx, |s, cx| s.text_length_utf16(window, cx)) {
            return length;
        }
        Some(self.input.text.encode_utf16().count())
    }
'''
CUT = '''            window,
            cx,
        );
    }
    pub(crate) fn key('''
FIRST = '    finish(client.cut_and_observe_selection());\n'
# Defers the same first query to the next native turn; the caret may look right.
LATER = '    let _ = client.cut_and_observe_selection_next_turn(finish);\n'
CASES = {
    'D6': (FIELD, LENGTH, '', 'p13-native-document-end'),
    'wrong-cut-caret': (FIELD, CUT, CUT.replace('        );\n    }', '        );\n        self.input.selected = 0..0;\n    }'), 'p13-native-cut-caret'),
    'cut-first-query-order': (QUERIES, FIRST, LATER, 'p13-cut-first-query-order'),
}
# The only legal milestone order (SDK NativeCutPhase); refusals carry no phase claim.
PHASES = ['Armed', 'AcceptedCut', 'DownReturned', 'UpReturned', 'OriginRestored', 'ModifiersRestored',
          'SnapshotEntered', 'SnapshotReturned', 'SelectorReturned', 'FirstGetterEntered', 'FirstResultStored']
TURN_CROSSINGS = 7


def pinned_sdk():
    """Cargo.toml's herdr-uikit rev without tomllib (the leased Mac runs 3.9)."""
    text = (ROOT / 'Cargo.toml').read_text()
    rows = re.findall(r'^"?herdr-uikit"?\s*=\s*\{[^}\n]*"?rev"?\s*=\s*"([0-9a-f]{40})"', text, re.M)
    return rows[0] if len(rows) == 1 else None


def load_base():
    spec = importlib.util.spec_from_file_location('p13_base', Path(__file__).with_name('p13-check.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def mutate(source, case):
    _, before, after, _ = CASES[case]
    if source.count(before) != 1:
        raise ValueError('native query mutation anchor is not unique: ' + case)
    return source.replace(before, after)


def verify_mutant(case):
    source = CASES[case][0]
    original = subprocess.check_output(['git', '-C', str(ROOT), 'show', 'HEAD:' + source])
    candidate = mutate(original.decode(), case).encode()
    changed = subprocess.check_output(['git', '-C', str(ROOT), 'diff', '--name-only', 'HEAD'], text=True).splitlines()
    untracked = subprocess.check_output(['git', '-C', str(ROOT), 'ls-files', '--others', '--exclude-standard'], text=True)
    if changed != [source] or untracked or (ROOT / source).read_bytes() != candidate:
        raise RuntimeError('Only the exact declared P13 query mutation may be collected')
    return {'case': case, 'source': source, 'original_sha256': hashlib.sha256(original).hexdigest(),
            'mutant_sha256': hashlib.sha256(candidate).hexdigest()}


def cut_order(row, client):
    """F1: one accepted actual Cut, then this receiver's FIRST selection query,
    with no deferral, turn crossing or other refusal in between."""
    cut = row.get('cut')
    if not isinstance(cut, dict) or cut.get('ok') is not True or cut.get('reason') is not None:
        raise ValueError('p13-cut-first-query-order: ' + repr(cut))
    if cut.get('selector_sent') is not True or cut.get('selector_returned') is not True or cut.get('client') != client:
        raise ValueError('p13-cut-first-query-order: selector/client ' + repr(cut))
    counters = cut.get('counters')
    if cut.get('refused') != 0 or not isinstance(counters, list) or len(counters) != 8 or counters[TURN_CROSSINGS] != 0:
        raise ValueError('p13-cut-first-query-order: refused/crossings ' + repr(cut))
    events = cut.get('events') or []
    if any(e.get('refusal') is not None for e in events):
        raise ValueError('p13-cut-first-query-order: refusal event')
    order = [PHASES.index(e.get('phase')) if e.get('phase') in PHASES else -1 for e in events]
    sequence = [e.get('sequence') for e in events]
    if -1 in order or order != sorted(set(order)) or sequence != sorted(set(sequence)):
        raise ValueError('p13-cut-first-query-order: phases ' + repr([e.get('phase') for e in events]))
    named = [e.get('phase') for e in events]
    if not {'AcceptedCut', 'SelectorReturned', 'FirstGetterEntered'} <= set(named) or named[-1] != 'FirstResultStored':
        raise ValueError('p13-cut-first-query-order: milestones ' + repr(named))
    return events[-1]


def sample(result, client, value, assertion):
    if (not isinstance(result, dict) or set(result) != {'client', 'value'}
            or type(result['client']) is not int or result['client'] != client
            or result['value'] != value or type(result['value']) is not type(value)):
        raise ValueError(assertion + ': ' + repr(result))


def observation(row, client, selected, length, assertion='p13-native-document-end'):
    if row.get('captured') != client:
        raise ValueError('p13-captured-client-identity')
    caret_assertion = 'p13-native-cut-caret' if row['label'] == 'Q03' else 'p13-native-selection'
    if row['label'] == 'Q03':
        # Order first: a later-correct caret never repairs a deferred first query.
        stored = cut_order(row, client)
        if stored.get('range') != [selected['start'], selected['end']]:
            raise ValueError(caret_assertion + ': stored ' + repr(stored))
    # The Cut caret MUST be checked before document/position and model state.
    sample(row.get('before'), client, selected, caret_assertion)
    sample(row.get('document'), client, length, assertion)
    sample(row.get('position'), client, length, 'p13-native-position')
    sample(row.get('beyond'), client, None, 'p13-native-position-nil')
    sample(row.get('after'), client, selected, 'p13-query-selection-unchanged')


def validate(records, messages):
    query_rows = [r for r in records if r.get('kind') == 'p13-native-query']
    if len(query_rows) != 5 or [r.get('label') for r in query_rows] != ['Q01', 'Q02', 'Q03', 'Q04', 'Q05-current']:
        raise ValueError('p13-query-coverage')
    old = query_rows[0].get('captured')
    if type(old) is not int or old < 0:
        raise ValueError('p13-captured-client-identity')
    expected = [('Q01', 3, 3, 6, 'abcdef', None),
                ('Q02', 5, 7, 13, 'pre:変😀中:post', {'start': 4, 'end': 8}),
                ('Q03', 5, 5, 11, 'pre:変中:post', {'start': 4, 'end': 6}),
                ('Q04', 4, 4, 9, 'pre::post', None)]
    for row, (label, start, end, length, text, marked) in zip(query_rows, expected):
        selected = {'start': start, 'end': end}
        observation(row, old, selected, length)
        if row.get('text') != text or row.get('marked') != marked or row.get('model_selection') != selected:
            raise ValueError('p13-query-consumer-state: ' + label)
        for result in row.get('setup', []):
            sample(result, old, True, 'p13-query-setup')
    if len(query_rows[0].get('setup', [])) != 1 or len(query_rows[1].get('setup', [])) != 3 or any('setup' in r for r in query_rows[2:]):
        raise ValueError('p13-query-setup-only')
    sample(query_rows[0].get('one'), old, 4, 'p13-native-position')
    if query_rows[0].get('overflow') != {'error': 'IntegerOutOfRange'}:
        raise ValueError('p13-query-numeric-error')
    if query_rows[0].get('clipboard') != 'p13-query-sentinel':
        raise ValueError('p13-query-clipboard-sentinel')
    if query_rows[2].get('clipboard') != '😀':
        raise ValueError('p13-native-cut-clipboard')
    if any(r.get('kind') == 'p13-query-cut-scheduled' for r in records):
        raise ValueError('p13-cut-first-query-order: timer-scheduled Cut')
    if not any(r.get('kind') == 'p13-query-escape' and r.get('handled') is True for r in records):
        raise ValueError('p13-native-escape-not-handled')
    current = query_rows[4]
    observation(current, old + 1, {'start': 6, 'end': 6}, 6)
    # A real current length6 positive precedes the retained old pair check.
    observation(current.get('old', {}), old, None, 0, 'p13-q05-retained-client-not-current')
    # Readiness is read on the first native turn after GPUI drew past the accepted ack.
    if current.get('settled') is not True:
        raise ValueError('p13-query-current-not-settled')
    if current.get('ready') != [True, True, True] or current.get('paused') is not False:
        raise ValueError('p13-query-current-not-ready')
    if not any(r.get('kind') == 'p13-native-retirement' and r.get('accepted') is True
               and r.get('retired') == old and r.get('replacement') == old + 1 for r in records):
        raise ValueError('p13-query-retirement-ack')
    done = [r for r in records if r.get('kind') == 'p13-query-done']
    if len(done) != 1 or done[0].get('captured') != old:
        raise ValueError('p13-query-done')
    load_base().barriers(records, ('query-old-after-close', 'query-fresh-after-close'),
                         'p13-query-barrier-before-flush')
    phase = 'search'
    seen, wire = [], []
    for message in messages:
        if 'ClientShellEndpointRequest' in message:
            req = json.loads(message['ClientShellEndpointRequest']['request'])
            label = req.get('params', {}).get('p13_barrier')
            if label:
                seen.append(label); phase = label
        if 'ClientShellPopupInput' in message:
            raise ValueError('p13-query-popup-wire')
        if 'ClientShellPaneInput' in message:
            payload = message['ClientShellPaneInput']
            if phase != 'query-old-after-close' or payload.get('pane_id') != 'w1:p1':
                raise ValueError('p13-query-stale-wire')
            wire.extend(payload['events'])
    if seen != ['query-before-close', 'query-old-after-close', 'query-fresh-after-close'] or wire != [{'TextCommit': 'Q05-fresh'}]:
        raise ValueError('p13-query-fresh-wire-count')
    print('P13 native query receipt validation PASS; provenance separate; physical IME/B1/B2 UNVERIFIED', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--run', action='store_true')
    parser.add_argument('--expected-head')
    parser.add_argument('--mutation-case', choices=CASES)
    args = parser.parse_args()
    directory = args.directory.resolve()
    if args.run:
        if not args.expected_head:
            parser.error('--run requires --expected-head')
        if pinned_sdk() != SDK or ('#' + SDK) not in (ROOT / 'Cargo.lock').read_text():
            raise RuntimeError('P13 query collector requires the reviewed isolated SDK helper')
        base = load_base()
        check = (lambda: verify_mutant(args.mutation_case)) if args.mutation_case else None
        base.collect(directory, args.expected_head, query_mode=True, verify_mutant=check)
        provenance = json.loads((directory / 'provenance.json').read_text())
        metadata = {'head': args.expected_head, 'sdk': SDK, 'mode': 'native-query',
                    'mutation': verify_mutant(args.mutation_case) if args.mutation_case else None,
                    'source_tree': provenance['tree'], 'changed': provenance['changed'],
                    'executable_sha256': provenance['installed']['executable_sha256'],
                    'compiler_fresh': provenance['compiler']['fresh']}
        (directory / 'query-provenance.json').write_text(json.dumps(metadata, indent=2) + '\n')
    records = [json.loads(line) for line in (directory / 'app.jsonl').read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / 'peer.jsonl').read_text().splitlines()]
    validate(records, messages)


if __name__ == '__main__':
    main()
