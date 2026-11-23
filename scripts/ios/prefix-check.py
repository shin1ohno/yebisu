#!/usr/bin/env python3
"""L57: run testHerdrPrefixInputPaths and print what the wire peer received per step.

Builds the Simulator app, installs it on HERDR_IOS_SIMULATOR, saves Shortcut style = Herdr
prefix in the app's settings file, runs the XCUITest, stops the app and only then reads
Documents/p4-peer.jsonl. The test types a marker letter after each step (SmokeQA+Prefix.swift),
so the log splits into steps S1 (key band ctrl, a, c), S2 (latch a twice), H0 (one plain
hardware key), H1 (hardware Control-A, c), H2 (Control-A twice) and H3 (Control-A, c again). Prints one PREFIX line per step and writes
summary.json with the screenshots into --out. Simulator results do not prove real IME or
hardware keyboard behavior (AGENTS.md).
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

STEPS = (('S1', 'z'), ('S2', 'y'), ('H0', '0'), ('H1', '1'), ('H2', '2'), ('H3', '3'))


def run(*command, **kwargs):
    print('PREFIX COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def events(record):
    """(kind, detail) for each pane input or endpoint request in one peer record. A record is
    serde's externally tagged ClientMessage: {"ClientShellPaneInput": {"events": [...]}}."""
    if not isinstance(record, dict) or len(record) != 1:
        return []
    (tag, body), = record.items()
    if not isinstance(body, dict):
        return []
    found = []
    if tag == 'ClientShellEndpointRequest':
        try:
            method = json.loads(body.get('request') or '{}').get('method')
        except ValueError:
            method = None
        found.append(('rpc', method))
    if tag == 'ClientShellPaneInput':
        for event in body.get('events') or []:
            if not isinstance(event, dict):
                continue
            if 'TextCommit' in event:
                found.append(('text', event['TextCommit']))
            elif isinstance(event.get('Key'), dict) and event['Key'].get('kind') == 'Press':
                found.append(('key', [event['Key'].get('code'), event['Key'].get('modifiers')]))
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--label', required=True)
    parser.add_argument('--command-control', action='store_true',
                        help='also save Terminal priority and Command as Control (L57 device result)')
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
    settings = container / 'Library/Application Support/herdr/gpui/yebisu-settings.json'
    settings.parent.mkdir(parents=True, exist_ok=True)
    # A partial document is refused ("invalid settings: invalid compact") and the defaults apply.
    document = {'version': 1, 'language': 'en-US', 'compact': False, 'terminal_priority': False,
                'board_hidden': False, 'drag_select': True, 'shortcut_style': 'herdr-prefix'}
    if args.command_control:
        document.update(terminal_priority=True, command_key='ctrl')
    settings.write_text(json.dumps(document) + '\n')
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    result = out / 'result.xcresult'
    if result.exists():
        shutil.rmtree(result)
    test = subprocess.run(['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
                           '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
                           '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO',
                           '-only-testing:SmokeOracle/SmokeQA/testHerdrPrefixInputPaths'],
                          text=True, capture_output=True)
    (out / 'xctest.stdout').write_text(test.stdout)
    (out / 'xctest.stderr').write_text(test.stderr)
    print('PREFIX xcodebuild exit', test.returncode, flush=True)
    for line in test.stdout.splitlines():
        if line.startswith('PREFIX') or 'error:' in line or 'Test Case' in line:
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
    summary = {'label': args.label, 'command_control': args.command_control,
               'xcodebuild_exit': test.returncode, 'records': len(records),
               'inputs': flat, 'steps': steps}
    (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    for name, items in steps.items():
        print('PREFIX', args.label, name, json.dumps(items), flush=True)


if __name__ == '__main__':
    main()
