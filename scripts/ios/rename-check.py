#!/usr/bin/env python3
"""Issue #143: drive rename in place on the Simulator and judge each step.

Builds the Simulator app with `test-support` (herdr-uikit's `native_inline_field_state`, the
`rename-state` records of shell/rename/qa.rs), installs it on HERDR_IOS_SIMULATOR, runs
SmokeQA/testRenameInPlace, stops the app and copies Documents/p0-smoke.jsonl (the app's records)
and Documents/p4-peer.jsonl (what the wire peer received). The test prints `RENAME_STEP <name>
<unix time>` before each step and types a marker letter into the terminal after each close, so the
records split by time and the peer log by marker (the same shape as prefix-check.py and
search-check.py).

Prints one `RENAME <step> PASS|FAIL <reason>` line per step, writes summary.json into --out and
exits non-zero on any failed step. Simulator results do not prove real IME, hardware keyboard,
Escape or Pencil behavior (AGENTS.md; docs/rename.md).
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

BUNDLE = 'com.shin1ohno.yebisu'
STEPS = ('R1', 'R2', 'R2b', 'R3', 'R4', 'R5')
MARKERS = {'R1': 'z', 'R2': 'y', 'R2b': 'u', 'R3': 'x', 'R4': 'w', 'R5': 'v'}
PASTED = 'renamepasteword'


def run(*command, **kwargs):
    print('RENAME COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def peer_events(record):
    """('rpc', method, params) for an endpoint request, ('text'|'paste'|'key', detail) for pane
    input, in one peer record (serde's externally tagged ClientMessage)."""
    if not isinstance(record, dict) or len(record) != 1:
        return []
    (tag, body), = record.items()
    if not isinstance(body, dict):
        return []
    if tag == 'ClientShellEndpointRequest':
        try:
            request = json.loads(body.get('request') or '{}')
        except ValueError:
            return []
        if not isinstance(request, dict):
            return []
        return [('rpc', request.get('method'), request.get('params'))]
    found = []
    if tag == 'ClientShellPaneInput':
        for event in body.get('events') or []:
            if not isinstance(event, dict):
                continue
            if 'TextCommit' in event:
                found.append(('text', event['TextCommit']))
            elif 'Paste' in event:
                found.append(('paste', event['Paste']))
            elif isinstance(event.get('Key'), dict) and event['Key'].get('kind') == 'Press':
                found.append(('key', event['Key'].get('code')))
    return found


def split_peer(records):
    """The peer's events before each step's marker, in step order; `rest` after the last."""
    flat = [item for record in records for item in peer_events(record)]
    steps, current, index = {}, [], 0
    order = [(step, MARKERS[step]) for step in STEPS]
    for item in flat:
        if index < len(order) and item[:2] == ('text', order[index][1]):
            steps[order[index][0]] = current
            current, index = [], index + 1
        else:
            current.append(item)
    steps['rest'] = current
    return flat, steps


def step_times(stdout):
    found = []
    for line in stdout.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[0] == 'RENAME_STEP':
            try:
                found.append((parts[1], float(parts[2])))
            except ValueError:
                continue
    return found


def by_step(records, times):
    states = [r for r in records if isinstance(r, dict) and r.get('kind') == 'rename-state'
              and isinstance(r.get('time'), (int, float))]
    windows = {}
    for index, (name, start) in enumerate(times):
        end = times[index + 1][1] if index + 1 < len(times) else float('inf')
        windows[name] = [r for r in states if start <= r['time'] < end]
    return windows


def renames(events, method=None):
    return [e for e in events if e[0] == 'rpc' and str(e[1]).endswith('.rename')
            and (method is None or e[1] == method)]


def inputs(events):
    return [e for e in events if e[0] != 'rpc']


def up(record):
    """The field owns first responder and neither GPUI view does."""
    sdk = record.get('sdk') or {}
    return (record.get('open') is True and sdk.get('field_first_responder') is True
            and sdk.get('gpui_text_first_responder') is False
            and sdk.get('gpui_drawing_first_responder') is False)


def cancelled_touches(records, times, name):
    """Touches UIKit cancelled during step `name` (a system gesture took them)."""
    bounds = dict(times)
    order = [step for step, _ in times]
    if name not in bounds:
        return 0
    at = order.index(name)
    end = times[at + 1][1] if at + 1 < len(times) else float('inf')
    return sum(1 for r in records if isinstance(r, dict) and r.get('kind') == 'input-receipt'
               and r.get('stage') == 'cancelled' and bounds[name] <= r.get('time', 0) < end)


def verdicts(windows, steps, cancelled=None):
    """(ok, reason) per step; ok is None (UNVERIFIED) for a double tap whose second touch UIKit
    cancelled: the Simulator then cannot show the gesture (device check D4)."""
    cancelled = cancelled or {}
    result = {}

    def check(name, ok, reason):
        result[name] = (bool(ok), reason)


    def opened(name):
        return any(up(r) for r in windows.get(name, []))

    r1 = steps.get('R1', [])
    check('R1', opened('R1') and [e[2] for e in renames(r1)] == [{'workspace_id': 'w1', 'label': 'ship'}]
          and not inputs(r1),
          f'opened={opened("R1")} renames={renames(r1)} inputs={inputs(r1)}')
    r2 = steps.get('R2', [])
    tab = renames(r2, 'tab.rename')
    check('R2', opened('R2') and len(tab) == 1 and (tab[0][2] or {}).get('label') == 'build'
          and len(renames(r2)) == 1 and not inputs(r2),
          f'opened={opened("R2")} renames={renames(r2)} inputs={inputs(r2)}')
    if not result['R2'][0] and not opened('R2') and cancelled.get('R2'):
        result['R2'] = (None, f'UIKit cancelled {cancelled["R2"]} touch(es) of the double tap at the top '
                              'of the window; not shown in this Simulator (device check D4)')
    r2b = steps.get('R2b', [])
    space = renames(r2b, 'workspace.rename')
    moves = [e for e in r2b if e[0] == 'rpc' and e[1] == 'workspace.focus']
    check('R2b', opened('R2b') and len(space) == 1 and (space[0][2] or {}).get('label') == 'space2'
          and len(moves) <= 1 and not inputs(r2b),
          f'opened={opened("R2b")} renames={renames(r2b)} moves={len(moves)} inputs={inputs(r2b)}')
    r3 = steps.get('R3', [])
    check('R3', opened('R3') and not renames(r3) and not inputs(r3),
          f'opened={opened("R3")} renames={renames(r3)} inputs={inputs(r3)}')
    r4 = steps.get('R4', [])
    leaked = [e for e in inputs(r4) if PASTED in str(e[1]) or e[1] in ('q', 'b')]
    check('R4', opened('R4') and not renames(r4) and not inputs(r4),
          f'opened={opened("R4")} renames={renames(r4)} inputs={inputs(r4)} leaked={leaked}')
    r5 = steps.get('R5', [])
    languages = sorted({(r.get('sdk') or {}).get('primary_language') for r in windows.get('R5', [])} - {None})
    marked = any((r.get('sdk') or {}).get('marked') for r in windows.get('R5', []))
    # Without a composition the first Return commits and closes, so the second reaches the
    # terminal after the restore: that one Return is expected, nothing else.
    extra = inputs(r5)
    expected_extra = [] if marked else [('key', 'Enter')]
    check('R5', opened('R5') and len(renames(r5, 'workspace.rename')) == 1 and extra in ([], expected_extra),
          f'opened={opened("R5")} renames={renames(r5)} languages={languages} marked_seen={marked} '
          f'(composition is recorded, not required: Simulator)')
    return result


def read_jsonl(path):
    records = []
    if path.is_file():
        for line in path.read_text().splitlines():
            try:
                records.append(json.loads(line))
            except ValueError:
                continue
    return records


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
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'],
                                       text=True)
    device = resolved.split()[0]
    subprocess.run(['xcrun', 'simctl', 'terminate', device, BUNDLE], text=True, capture_output=True)
    run('xcrun', 'simctl', 'install', device, app_path)
    container = Path(subprocess.check_output(['xcrun', 'simctl', 'get_app_container', device, BUNDLE, 'data'],
                                             text=True).strip())
    documents = container / 'Documents'
    for leftover in ('p4-peer.jsonl', 'p0-smoke.jsonl'):
        if (documents / leftover).exists():
            (documents / leftover).unlink()
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    result = out / 'result.xcresult'
    if result.exists():
        shutil.rmtree(result)
    test = subprocess.run(['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
                           '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
                           '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO',
                           '-only-testing:SmokeOracle/SmokeQA/testRenameInPlace'],
                          text=True, capture_output=True)
    (out / 'xctest.stdout').write_text(test.stdout)
    (out / 'xctest.stderr').write_text(test.stderr)
    print('RENAME xcodebuild exit', test.returncode, flush=True)
    subprocess.run(['xcrun', 'simctl', 'terminate', device, BUNDLE], text=True, capture_output=True)
    for source, target in (('p4-peer.jsonl', 'peer.jsonl'), ('p0-smoke.jsonl', 'app.jsonl')):
        if (documents / source).is_file():
            shutil.copyfile(documents / source, out / target)
    records, peer = read_jsonl(out / 'app.jsonl'), read_jsonl(out / 'peer.jsonl')
    times = step_times(test.stdout)
    windows = by_step(records, times)
    flat, steps = split_peer(peer)
    cancelled = {name: cancelled_touches(records, times, name) for name in STEPS}
    outcome = verdicts(windows, steps, cancelled)
    summary = {
        'label': args.label,
        'xcodebuild_exit': test.returncode,
        'records': len(records),
        'peer_events': flat,
        'peer_steps': steps,
        'steps': {name: {'ok': ok, 'reason': reason, 'records': len(windows.get(name, []))}
                  for name, (ok, reason) in outcome.items()},
    }
    (out / 'summary.json').write_text(json.dumps(summary, indent=2, default=str) + '\n')
    failed = False
    for name, (ok, reason) in outcome.items():
        print('RENAME', name, {True: 'PASS', False: 'FAIL', None: 'UNVERIFIED'}[ok], reason, flush=True)
        failed |= ok is False
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
