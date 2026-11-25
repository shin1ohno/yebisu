#!/usr/bin/env python3
"""Issue #137: run testTerminalScroll and check what the wire peer received per step.

Builds the Simulator app, installs it on HERDR_IOS_SIMULATOR, runs the XCUITest
(SmokeQA+Scroll.swift), stops the app and only then reads Documents/p4-peer.jsonl. The test
types a marker key after each step, so the log splits into steps s (a slow 300 pt drag down),
t (a fast swipe up that coasts after the lift) and u (a sideways drag). Pass: s sends only
ScrollUp, 10-20 lines in all; t sends ScrollDown in more events than s did; u sends no wheel
input. Prints one SCROLL line per step, writes summary.json into --out and exits 1 when a step
fails. Simulator results do not prove real trackpad, wheel or IME behavior (AGENTS.md).
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

STEPS = (('s', 's'), ('t', 't'), ('u', 'u'))


def run(*command, **kwargs):
    print('SCROLL COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def events(record):
    """('mouse', kind, lines) for each wheel event and ('text', text) for each typed text in one
    peer record, serde's externally tagged ClientMessage: {"ClientShellPaneInput": {...}}."""
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
            found.append(('text', event['TextCommit']))
        elif isinstance(event.get('Mouse'), dict):
            mouse = event['Mouse']
            found.append(('mouse', mouse.get('kind'), mouse.get('lines')))
    return found


def split(records):
    """The peer's inputs per step, cut at each step's marker text."""
    flat = [item for record in records for item in events(record)]
    steps, current, index = {}, [], 0
    for item in flat:
        if index < len(STEPS) and item == ('text', STEPS[index][1]):
            steps[STEPS[index][0]] = current
            current, index = [], index + 1
        else:
            current.append(item)
    steps['rest'] = current
    return flat, steps


def wheel(items):
    return [item for item in items if item[0] == 'mouse']


def verdicts(steps):
    """Each step's pass/fail and the reason, from the per-step inputs."""
    s, t, u = (wheel(steps.get(name) or []) for name in ('s', 't', 'u'))
    lines = sum(item[2] or 0 for item in s)
    result = {}
    result['s'] = (bool(s) and all(item[1] == 'ScrollUp' for item in s) and 10 <= lines <= 20,
                   '%d events, %d lines, kinds %s' % (len(s), lines, sorted({item[1] for item in s})))
    downs = [item for item in t if item[1] == 'ScrollDown']
    result['t'] = (bool(downs) and len(t) > len(s),
                   '%d events (%d ScrollDown) against s %d' % (len(t), len(downs), len(s)))
    result['u'] = (not u and 'u' in steps, '%d wheel events' % len(u))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--label', required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    app = run('bash', 'scripts/ios/bundle.sh', '--package', 'yebisu', '--bin', 'yebisu', capture_output=True)
    print(app.stderr[-4000:], flush=True)
    app_path = app.stdout.strip().splitlines()[-1]
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], text=True)
    device = resolved.split()[0]
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    run('xcrun', 'simctl', 'install', device, app_path)
    container = Path(subprocess.check_output(['xcrun', 'simctl', 'get_app_container', device, 'com.shin1ohno.yebisu', 'data'], text=True).strip())
    peer = container / 'Documents/p4-peer.jsonl'
    if peer.exists():
        peer.unlink()
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    result = out / 'result.xcresult'
    if result.exists():
        shutil.rmtree(result)
    test = subprocess.run(['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
                           '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
                           '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO',
                           '-only-testing:SmokeOracle/SmokeQA/testTerminalScroll'],
                          text=True, capture_output=True)
    (out / 'xctest.stdout').write_text(test.stdout)
    (out / 'xctest.stderr').write_text(test.stderr)
    print('SCROLL xcodebuild exit', test.returncode, flush=True)
    for line in test.stdout.splitlines():
        if line.startswith('SCROLL') or 'error:' in line or 'Test Case' in line:
            print(line, flush=True)
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    records = []
    if peer.is_file():
        shutil.copyfile(peer, out / 'p4-peer.jsonl')
        for line in peer.read_text().splitlines():
            try:
                records.append(json.loads(line))
            except ValueError:
                continue
    flat, steps = split(records)
    export = out / 'attachments'
    if export.exists():
        shutil.rmtree(export)
    subprocess.run(['xcrun', 'xcresulttool', 'export', 'attachments', '--path', str(result), '--output-path', str(export)],
                   text=True, capture_output=True)
    manifest = export / 'manifest.json'
    if manifest.is_file():
        for case in json.loads(manifest.read_text()):
            for attachment in case['attachments']:
                name = attachment['suggestedHumanReadableName'].split('_0_')[0]
                shutil.copyfile(export / attachment['exportedFileName'], out / (name + '.png'))
    checked = verdicts(steps)
    summary = {'label': args.label, 'xcodebuild_exit': test.returncode, 'records': len(records),
               'inputs': flat, 'steps': steps,
               'verdicts': {name: {'pass': ok, 'detail': detail} for name, (ok, detail) in checked.items()}}
    (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    for name, (ok, detail) in checked.items():
        print('SCROLL', args.label, name, 'PASS' if ok else 'FAIL', detail, json.dumps(steps.get(name)), flush=True)
    return 0 if test.returncode == 0 and all(ok for ok, _ in checked.values()) else 1


if __name__ == '__main__':
    sys.exit(main())
