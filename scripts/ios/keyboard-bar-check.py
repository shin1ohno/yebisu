#!/usr/bin/env python3
"""#229: run SmokeQA+KeyboardBar.swift on HERDR_IOS_SIMULATOR and keep its evidence.

Builds the Simulator app, installs it, runs testKeyboardBarStates (docked bar, the … menu,
floating band, redock) and testKeyboardBarHardwareOnly (the negative control), then writes
into --out: xctest.stdout/stderr, the KBAR lines (kbar.txt), the screenshots
(kbar-<step>.png) and summary.json. Exits with xcodebuild's status. A split keyboard is an
owner device check (herdr-uikit docs/keyboard-bar.md). Simulator results do not prove real IME
or hardware keyboard behaviour (AGENTS.md).
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

TESTS = ('testKeyboardBarStates', 'testKeyboardBarHardwareOnly')


def run(*command, **kwargs):
    print('KBAR COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def kbar_lines(stdout):
    """The test's KBAR lines and XCTest's case results, in order."""
    return [line for line in stdout.splitlines()
            if line.startswith('KBAR') or 'error:' in line or line.startswith("Test Case '")]


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
    print(app.stderr[-2000:], flush=True)
    app_path = app.stdout.strip().splitlines()[-1]
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'],
                                       text=True)
    device = resolved.split()[0]
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    run('xcrun', 'simctl', 'install', device, app_path)
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    result = out / 'result.xcresult'
    if result.exists():
        shutil.rmtree(result)
    only = [f'-only-testing:SmokeOracle/SmokeQA/{name}' for name in TESTS]
    test = subprocess.run(['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
                           '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
                           '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO', *only],
                          text=True, capture_output=True)
    (out / 'xctest.stdout').write_text(test.stdout)
    (out / 'xctest.stderr').write_text(test.stderr)
    lines = kbar_lines(test.stdout)
    (out / 'kbar.txt').write_text('\n'.join(lines) + '\n')
    for line in lines:
        print(line, flush=True)
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    export = out / 'attachments'
    if export.exists():
        shutil.rmtree(export)
    subprocess.run(['xcrun', 'xcresulttool', 'export', 'attachments', '--path', str(result), '--output-path', str(export)],
                   text=True, capture_output=True)
    screenshots = []
    manifest = export / 'manifest.json'
    if manifest.is_file():
        for case in json.loads(manifest.read_text()):
            for attachment in case['attachments']:
                name = attachment['suggestedHumanReadableName'].split('_0_')[0]
                shutil.copyfile(export / attachment['exportedFileName'], out / (name + '.png'))
                screenshots.append(name)
    summary = {'label': args.label, 'xcodebuild_exit': test.returncode, 'tests': list(TESTS),
               'screenshots': screenshots, 'kbar': lines}
    (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print('KBAR xcodebuild exit', test.returncode, flush=True)
    return test.returncode


if __name__ == '__main__':
    sys.exit(main())
