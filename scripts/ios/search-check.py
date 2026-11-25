#!/usr/bin/env python3
"""#138: drive the sidebar's native search panel on the Simulator and judge each step.

Builds the Simulator app with `test-support` (herdr-uikit's `native_search_state`, the
`search-state` records of shell/search_qa.rs), installs it on HERDR_IOS_SIMULATOR, runs
SmokeQA/testSidebarSearch, then SmokeQA/testSidebarSearchCompact (YEBISU_QA_SEARCH_COMPACT=1:
the Simulator has no window below 352 pt). After each run it stops the app and copies
Documents/p0-smoke.jsonl (the app's records) and Documents/p4-peer.jsonl (what the wire peer
received). The test prints `SEARCH_STEP <name> <unix time>` before each step and types a marker
letter into the terminal after the steps that close the panel, so the records split by time and
the peer log by marker. XCUITest element queries may not answer while the panel is up
(herdr-uikit scripts/native-search-qa/Tests/NativeSearch.swift:5-7), so the verdicts come from
these files after the run, not from qa.state reads during it.

Prints one `SEARCH <step> PASS|FAIL <reason>` line per step, writes summary.json into --out and
exits non-zero on any failed step. Simulator results do not prove real IME, hardware keyboard,
Universal Clipboard, dictation, Escape, an abandoned sheet swipe or a real narrow window
(AGENTS.md; docs/ci-simulator.md "Sidebar search").
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

BUNDLE = 'com.shin1ohno.yebisu'
STEPS = ('T-S1', 'T-S2', 'T-S3', 'T-S4', 'T-S5', 'T-S6', 'T-S7', 'T-S8')
# Marker letters the test types into the terminal once the panel closed, per step.
MARKERS = {'T-S3': 'z', 'T-S4': 'w', 'T-S6': 'x', 'T-S7': 'v', 'T-S8': 'u'}
PASTED = 'searchpasteword'


def run(*command, **kwargs):
    print('SEARCH COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def step_times(stdout):
    """[(step, unix time)] in order, from the test's `SEARCH_STEP` lines."""
    found = []
    for line in stdout.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[0] == 'SEARCH_STEP':
            try:
                found.append((parts[1], float(parts[2])))
            except ValueError:
                continue
    return found


def by_step(records, times):
    """The `search-state` records of each step: from its start to the next step's start."""
    states = [r for r in records if isinstance(r, dict) and r.get('kind') == 'search-state'
              and isinstance(r.get('time'), (int, float))]
    windows = {}
    for index, (name, start) in enumerate(times):
        end = times[index + 1][1] if index + 1 < len(times) else float('inf')
        windows[name] = [r for r in states if start <= r['time'] < end]
    return windows


def step_events(records, times):
    """{step: [(time, event)]}: each numbered event of the records' `events` history, in the step
    whose window holds the first record that carries it. A `Dismissed` and the `InputRestored`
    after it can land in one frame, so `last_event` alone can skip the dismissal."""
    first = {}
    states = sorted((r for r in records if isinstance(r, dict) and r.get('kind') == 'search-state'
                     and isinstance(r.get('time'), (int, float))), key=lambda r: r['time'])
    for record in states:
        for entry in record.get('events') or []:
            if (isinstance(entry, list) and len(entry) == 2 and isinstance(entry[0], int)
                    and isinstance(entry[1], str)):
                first.setdefault(entry[0], (record['time'], entry[1]))
    steps = {name: [] for name, _ in times}
    for number in sorted(first):
        time, event = first[number]
        for index, (name, start) in enumerate(times):
            end = times[index + 1][1] if index + 1 < len(times) else float('inf')
            if start <= time < end:
                steps[name].append((time, event))
    return steps


def peer_events(record):
    """(kind, detail) for each pane input in one peer record (externally tagged ClientMessage)."""
    if not isinstance(record, dict) or len(record) != 1:
        return []
    (tag, body), = record.items()
    if tag != 'ClientShellPaneInput' or not isinstance(body, dict):
        return []
    found = []
    for event in body.get('events') or []:
        if not isinstance(event, dict):
            continue
        if 'TextCommit' in event:
            found.append(('text', event['TextCommit'], body.get('pane_id')))
        elif 'Paste' in event:
            found.append(('paste', event['Paste'], body.get('pane_id')))
        elif isinstance(event.get('Key'), dict) and event['Key'].get('kind') == 'Press':
            found.append(('key', event['Key'].get('code'), body.get('pane_id')))
    return found


def split_peer(records, markers):
    """The peer's inputs before each marker, in marker order; `rest` is what came after the last."""
    flat = [item for record in records for item in peer_events(record)]
    order = list(markers.items())
    steps, current, index = {}, [], 0
    for item in flat:
        if index < len(order) and item[:2] == ('text', order[index][1]):
            steps[order[index][0]] = current
            current, index = [], index + 1
        else:
            current.append(item)
    steps['rest'] = current
    return flat, steps


def sdk(record, key):
    return (record.get('sdk') or {}).get(key)


def up(record):
    """The panel's field owns first responder and neither GPUI view does."""
    return (record.get('open') and record.get('phase') == 'Open' and sdk(record, 'field_first_responder') is True
            and sdk(record, 'gpui_text_first_responder') is False
            and sdk(record, 'gpui_drawing_first_responder') is False)


def state_at(records, time):
    """The last record at or before `time`: records are written only when the state changes."""
    earlier = [r for r in records if r.get('time', 0) <= time]
    return max(earlier, key=lambda r: r['time']) if earlier else {}


def count_text(flat, text):
    return sum(1 for item in flat if item[:2] == ('text', text))


def verdicts(windows, peer_flat, peer_steps, compact_windows, stdout, events=None):
    """{step: (ok, reason)} for every step; `events` is step_events() of the main run."""
    result = {}
    events = events or {}

    def check(name, ok, reason):
        result[name] = (bool(ok), reason)

    def any_in(name, predicate):
        return any(predicate(r) for r in windows.get(name, []))

    def seen(name, prefix):
        return any(event.startswith(prefix) for _, event in events.get(name, []))

    check('T-S1', any_in('T-S1', lambda r: up(r) and sdk(r, 'presentation_style') == 7),
          'popover (style 7) up, field first responder, GPUI views not')
    check('T-S2', any_in('T-S2', lambda r: up(r) and sdk(r, 'field_text') == 'pro' and r.get('filter') == 'pro')
          and peer_steps.get('T-S3') == [],
          'field and filter "pro" while up; the peer received nothing before the T-S3 marker')
    check('T-S3', seen('T-S3', 'Dismissed(Return)')
          and any_in('T-S3', lambda r: not r.get('open') and r.get('filter') == 'pro')
          and 'SEARCH banner exists=true' in stdout and count_text(peer_flat, MARKERS['T-S3']) == 1,
          'closed by Return with the filter kept, the banner shown, the marker received once')
    check('T-S4', seen('T-S4', 'Dismissed(Cancel)') and seen('T-S4', 'Dismissed(OutsideTap)')
          and not any(r.get('filter') != 'pro' for r in windows.get('T-S4', []))
          and not peer_steps.get('T-S4') and count_text(peer_flat, MARKERS['T-S4']) == 1,
          'Cancel and an outside tap closed it with the filter kept; nothing else reached the peer')
    s5 = windows.get('T-S5', [])
    cleared = _after(stdout, 'clear')
    check('T-S5', any(up(r) and PASTED in str(sdk(r, 'field_text')) for r in s5)
          and len({r.get('sidebar_mode') for r in s5}) <= 1 and not any(r.get('find_open') for r in s5)
          and not any(PASTED in str(i[1]) for i in peer_flat)
          and seen('T-S5', 'ClearRequested')
          and any(up(r) and sdk(r, 'field_text') == '' and r.get('filter') == '' for r in s5
                  if r.get('time', 0) >= cleared),
          'Command-V into the field; Command-B and Command-F changed nothing; no paste reached the peer; '
          'Clear filter in the empty results emptied the field and the filter')
    check('T-S6', seen('T-S6', 'Selected {') and seen('T-S6', 'Dismissed(Selected(')
          and count_text(peer_flat, MARKERS['T-S6']) == 1 and not peer_steps.get('T-S6'),
          'a result row opened, the panel closed for it, the marker reached the terminal once')
    check('T-S7', any_in('T-S7', up) and seen('T-S7', 'Dismissed(Cancel)') and not peer_steps.get('T-S7') and count_text(peer_flat, MARKERS['T-S7']) == 1,
          'the terminal composition at open was not sent; the marker after the close arrived once')
    s8 = windows.get('T-S8', [])
    foreground, close = _after(stdout, 'foreground'), _after(stdout, 'close')
    hidden = state_at(s8, close)
    check('T-S8', up(state_at(s8, foreground))
          and hidden.get('phase') == 'Open' and sdk(hidden, 'phase') == 'Presented'
          and not any(event.startswith('Dismissed') for time, event in events.get('T-S8', []) if time < close)
          and seen('T-S8', 'Dismissed(Cancel)') and count_text(peer_flat, MARKERS['T-S8']) == 1,
          'after background and foreground the field is first responder; the keyboard\'s hide key '
          'leaves the panel up; Cancel then closes it')
    check('T-S9', any(up(r) and sdk(r, 'presentation_style') == 1 for r in compact_windows.get('T-S9', [])),
          'with YEBISU_QA_SEARCH_COMPACT=1 the panel is a page sheet (style 1)')
    return result


def _after(stdout, mark):
    """The unix time of the test's `SEARCH_MARK <mark> <time>` line, or +inf."""
    for line in stdout.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[0] == 'SEARCH_MARK' and parts[1] == mark:
            try:
                return float(parts[2])
            except ValueError:
                break
    return float('inf')


def read_jsonl(path):
    records = []
    if path.is_file():
        for line in path.read_text().splitlines():
            try:
                records.append(json.loads(line))
            except ValueError:
                continue
    return records


def xctest(root, spec, device, out, name, env):
    result = out / f'{name}.xcresult'
    if result.exists():
        shutil.rmtree(result)
    test = subprocess.run(['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
                           '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
                           '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO',
                           f'-only-testing:SmokeOracle/SmokeQA/{name}'],
                          text=True, capture_output=True, env={**os.environ, **env})
    (out / f'{name}.stdout').write_text(test.stdout)
    (out / f'{name}.stderr').write_text(test.stderr)
    print('SEARCH xcodebuild', name, 'exit', test.returncode, flush=True)
    for line in test.stdout.splitlines():
        if line.startswith('SEARCH') or 'error:' in line or 'Test Case' in line:
            print(line, flush=True)
    return test


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--label', required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    app = run('bash', 'scripts/ios/bundle.sh', '--package', 'yebisu', '--bin', 'yebisu', '--features', 'test-support',
              capture_output=True)
    print(app.stderr[-4000:], flush=True)
    app_path = app.stdout.strip().splitlines()[-1]
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], text=True)
    device = resolved.split()[0]
    subprocess.run(['xcrun', 'simctl', 'terminate', device, BUNDLE], text=True, capture_output=True)
    run('xcrun', 'simctl', 'install', device, app_path)
    container = Path(subprocess.check_output(['xcrun', 'simctl', 'get_app_container', device, BUNDLE, 'data'], text=True).strip())
    documents = container / 'Documents'
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    runs = {}
    for name, env in (('testSidebarSearch', {}), ('testSidebarSearchCompact', {})):
        for leftover in ('p4-peer.jsonl', 'p0-smoke.jsonl'):
            if (documents / leftover).exists():
                (documents / leftover).unlink()
        test = xctest(root, spec, device, out, name, env)
        subprocess.run(['xcrun', 'simctl', 'terminate', device, BUNDLE], text=True, capture_output=True)
        for source, target in (('p4-peer.jsonl', f'{name}-peer.jsonl'), ('p0-smoke.jsonl', f'{name}-app.jsonl')):
            if (documents / source).is_file():
                shutil.copyfile(documents / source, out / target)
        runs[name] = (test, read_jsonl(out / f'{name}-app.jsonl'), read_jsonl(out / f'{name}-peer.jsonl'))
    main_test, records, peer = runs['testSidebarSearch']
    compact_test, compact_records, _ = runs['testSidebarSearchCompact']
    times = step_times(main_test.stdout)
    windows = by_step(records, times)
    compact_windows = by_step(compact_records, step_times(compact_test.stdout))
    flat, steps = split_peer(peer, MARKERS)
    events = step_events(records, times)
    result = verdicts(windows, flat, steps, compact_windows, main_test.stdout, events)
    summary = {
        'label': args.label,
        'xcodebuild_exit': {name: test.returncode for name, (test, _, _) in runs.items()},
        'records': {name: len(rows) for name, (_, rows, _) in runs.items()},
        'peer_inputs': flat,
        'peer_steps': steps,
        'events': events,
        'steps': {name: {'ok': ok, 'reason': reason, 'records': len(windows.get(name, compact_windows.get(name, [])))}
                  for name, (ok, reason) in result.items()},
    }
    (out / 'summary.json').write_text(json.dumps(summary, indent=2, default=str) + '\n')
    failed = False
    for name, (ok, reason) in result.items():
        print('SEARCH', name, 'PASS' if ok else 'FAIL', reason, flush=True)
        failed |= not ok
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
