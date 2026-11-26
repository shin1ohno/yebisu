#!/usr/bin/env python3
"""#231: run SmokeQA+SpaceMenu.swift and count what the receiving side got.

Builds the Simulator app, installs it on HERDR_IOS_SIMULATOR, runs testSpaceMenuHerdr1 and
testSpaceMenuHerdr2 (each judges its steps from qa.state), stops the app and only then reads its
receipts: the Herdr 1 wire peer's Documents/p4-peer.jsonl and the mock core's
Documents/p9-core-wire.jsonl. Pass: the peer got exactly one workspace.close, for w2, and one
workspace.rename; the core saved exactly three doc.set (beta closed, Build renamed, Build
closed), each accepted; both tests ran (their final screenshots are in the result). Writes
summary.json into --out and exits 1 on any failure. Simulator results do not prove a real
trackpad's secondary click or touch feel (AGENTS.md).

  space-menu-check.py --out DIR --label NAME
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

TESTS = ('testSpaceMenuHerdr1', 'testSpaceMenuHerdr2')
SHOTS = ('space-menu-h1', 'space-menu-h2')
DOC_SETS = ('ok', 'ok', 'ok')


def run(*command, **kwargs):
    print('SPACE MENU COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def read_jsonl(path):
    records = []
    if path.is_file():
        for line in path.read_text().splitlines():
            try:
                records.append(json.loads(line))
            except ValueError:
                continue
    return records


def peer_requests(records):
    """(method, params) of each endpoint request in the peer's log, serde's externally tagged
    ClientMessage: {"ClientShellEndpointRequest": {"request": "<json>", ...}}."""
    found = []
    for record in records:
        if not isinstance(record, dict) or len(record) != 1:
            continue
        (tag, body), = record.items()
        if tag != 'ClientShellEndpointRequest' or not isinstance(body, dict):
            continue
        try:
            request = json.loads(body.get('request') or '')
        except ValueError:
            continue
        if isinstance(request, dict):
            found.append((request.get('method'), request.get('params')))
    return found


def verdicts(peer, wire):
    """Each receiving-side check's pass/fail and detail."""
    requests = peer_requests(peer)
    closes = [params for method, params in requests if method == 'workspace.close']
    renames = [params for method, params in requests if method == 'workspace.rename']
    sets = tuple(receipt.get('result') for receipt in wire if receipt.get('method') == 'doc.set')
    return {
        'herdr1-close': (closes == [{'workspace_id': 'w2'}], json.dumps(closes)),
        'herdr1-rename': (len(renames) == 1, json.dumps(renames)),
        'herdr2-doc-set': (sets == DOC_SETS, json.dumps(sets)),
    }


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
    peer_path = container / 'Documents/p4-peer.jsonl'
    wire_path = container / 'Documents/p9-core-wire.jsonl'
    for path in (peer_path, wire_path):
        if path.exists():
            path.unlink()
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    result = out / 'result.xcresult'
    if result.exists():
        shutil.rmtree(result)
    command = ['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
               '-test-timeouts-enabled', 'YES',
               '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
               '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO']
    command += ['-only-testing:SmokeOracle/SmokeQA/%s()' % name for name in TESTS]
    test = subprocess.run(command, text=True, capture_output=True)
    (out / 'xctest.stdout').write_text(test.stdout)
    (out / 'xctest.stderr').write_text(test.stderr)
    print('SPACE MENU xcodebuild exit', test.returncode, flush=True)
    for line in test.stdout.splitlines():
        if line.startswith(('SPACE MENU', 'QAAX press')) or 'error:' in line or 'Test Case' in line:
            print(line, flush=True)
    # The app is stopped before its receipts are read.
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    peer, wire = read_jsonl(peer_path), read_jsonl(wire_path)
    for path in (peer_path, wire_path):
        if path.is_file():
            shutil.copyfile(path, out / path.name)
    export = out / 'attachments'
    if export.exists():
        shutil.rmtree(export)
    subprocess.run(['xcrun', 'xcresulttool', 'export', 'attachments', '--path', str(result), '--output-path', str(export)],
                   text=True, capture_output=True)
    shots = set()
    manifest = export / 'manifest.json'
    if manifest.is_file():
        for case in json.loads(manifest.read_text()):
            for attachment in case['attachments']:
                name = attachment['suggestedHumanReadableName'].split('_0_')[0]
                shots.add(name)
                shutil.copyfile(export / attachment['exportedFileName'], out / (name + '.png'))
    checked = verdicts(peer, wire)
    for shot in SHOTS:
        checked['ran-' + shot] = (shot in shots, 'attachment %s' % ('found' if shot in shots else 'missing'))
    summary = {'label': args.label, 'xcodebuild_exit': test.returncode, 'peer_records': len(peer),
               'wire_records': len(wire),
               'verdicts': {name: {'pass': ok, 'detail': detail} for name, (ok, detail) in checked.items()}}
    (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    for name, (ok, detail) in checked.items():
        print('SPACE MENU', args.label, name, 'PASS' if ok else 'FAIL', detail, flush=True)
    return 0 if test.returncode == 0 and all(ok for ok, _ in checked.values()) else 1


if __name__ == '__main__':
    sys.exit(main())
