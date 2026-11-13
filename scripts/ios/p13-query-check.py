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
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SDK = 'fe3a0da1d1a9c2d52a94510c4d4f7302339cd5f7'
SOURCE = 'crates/yebisu/src/find/field.rs'
LENGTH = '''    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.input.text.encode_utf16().count())
    }
'''
CUT = '''            window,
            cx,
        );
    }
    pub(crate) fn key('''
CASES = {
    'D6': (LENGTH, '', 'p13-native-document-end'),
    'wrong-cut-caret': (CUT, CUT.replace('        );\n    }', '        );\n        self.input.selected = 0..0;\n    }'), 'p13-native-cut-caret'),
}


def load_base():
    spec = importlib.util.spec_from_file_location('p13_base', Path(__file__).with_name('p13-check.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def mutate(source, case):
    before, after, _ = CASES[case]
    if source.count(before) != 1:
        raise ValueError('native query mutation anchor is not unique: ' + case)
    return source.replace(before, after)


def verify_mutant(case):
    original = subprocess.check_output(['git', '-C', str(ROOT), 'show', 'HEAD:' + SOURCE])
    candidate = mutate(original.decode(), case).encode()
    changed = subprocess.check_output(['git', '-C', str(ROOT), 'diff', '--name-only', 'HEAD'], text=True).splitlines()
    untracked = subprocess.check_output(['git', '-C', str(ROOT), 'ls-files', '--others', '--exclude-standard'], text=True)
    if changed != [SOURCE] or untracked or (ROOT / SOURCE).read_bytes() != candidate:
        raise RuntimeError('Only the exact declared P13 query mutation may be collected')
    return {'case': case, 'original_sha256': hashlib.sha256(original).hexdigest(),
            'mutant_sha256': hashlib.sha256(candidate).hexdigest()}


def sample(result, client, value, assertion):
    if (not isinstance(result, dict) or set(result) != {'client', 'value'}
            or type(result['client']) is not int or result['client'] != client
            or result['value'] != value or type(result['value']) is not type(value)):
        raise ValueError(assertion + ': ' + repr(result))


def observation(row, client, selected, length, assertion='p13-native-document-end'):
    if row.get('captured') != client:
        raise ValueError('p13-captured-client-identity')
    caret_assertion = 'p13-native-cut-caret' if row['label'] == 'Q03' else 'p13-native-selection'
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
    if query_rows[2].get('clipboard') != '😀':
        raise ValueError('p13-native-cut-clipboard')
    if not any(r.get('kind') == 'p13-query-cut-scheduled' and r.get('scheduled') is True for r in records):
        raise ValueError('p13-native-cut-not-scheduled')
    if not any(r.get('kind') == 'p13-query-escape' and r.get('handled') is True for r in records):
        raise ValueError('p13-native-escape-not-handled')
    current = query_rows[4]
    observation(current, old + 1, {'start': 6, 'end': 6}, 6)
    # A real current length6 positive precedes the retained old pair check.
    observation(current.get('old', {}), old, None, 0, 'p13-q05-retained-client-not-current')
    if current.get('ready') != [True, True, True] or current.get('paused') is not False:
        raise ValueError('p13-query-current-not-ready')
    if not any(r.get('kind') == 'p13-native-retirement' and r.get('accepted') is True
               and r.get('retired') == old and r.get('replacement') == old + 1 for r in records):
        raise ValueError('p13-query-retirement-ack')
    done = [r for r in records if r.get('kind') == 'p13-query-done']
    if len(done) != 1 or done[0].get('captured') != old:
        raise ValueError('p13-query-done')
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
        pin = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['dependencies']['herdr-uikit']['rev']
        if pin != SDK or ('#' + SDK) not in (ROOT / 'Cargo.lock').read_text():
            raise RuntimeError('P13 query collector requires the reviewed isolated SDK helper')
        base = load_base()
        check = (lambda: verify_mutant(args.mutation_case)) if args.mutation_case else None
        base.collect(directory, args.expected_head, query_mode=True, verify_mutant=check)
        metadata = {'head': args.expected_head, 'sdk': SDK, 'mode': 'native-query', 'mutation': verify_mutant(args.mutation_case) if args.mutation_case else None}
        (directory / 'query-provenance.json').write_text(json.dumps(metadata, indent=2) + '\n')
    records = [json.loads(line) for line in (directory / 'app.jsonl').read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / 'peer.jsonl').read_text().splitlines()]
    validate(records, messages)


if __name__ == '__main__':
    main()
