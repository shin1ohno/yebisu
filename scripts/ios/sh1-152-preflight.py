#!/usr/bin/env python3
"""Collect real UIKit P1-P4 evidence before choosing the SH1-152 adapter."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    print('PREFLIGHT COMMAND', list(args), flush=True)
    return subprocess.run(args, check=True, text=True, **kwargs)


def main():
    spec = importlib.util.spec_from_file_location('ci_smoke', Path(__file__).with_name('ci-smoke.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    listing = subprocess.check_output(['xcrun', 'simctl', 'list', '--json', 'devices', 'available'], text=True)
    device_type, runtime = module.select_template(json.loads(listing)['devices'])
    device = subprocess.check_output(['xcrun', 'simctl', 'create', 'SH1-152 preflight', device_type, runtime], text=True).strip()
    destination = ROOT / 'target/ios-sh1-152-preflight' / time.strftime('%Y%m%d-%H%M%S')
    destination.mkdir(parents=True)
    container = None
    try:
        environment = {**os.environ, 'HERDR_IOS_SIMULATOR': device}
        bundle = run('bash', 'scripts/ios/bundle.sh', '--package', 'yebisu', '--bin', 'yebisu',
                     cwd=ROOT, env=environment, capture_output=True)
        print(bundle.stderr, flush=True)
        run('xcrun', 'simctl', 'bootstatus', device, '-b')
        run('xcrun', 'simctl', 'install', device, bundle.stdout.strip().splitlines()[-1])
        container = Path(subprocess.check_output(['xcrun', 'simctl', 'get_app_container', device, 'com.shin1ohno.yebisu', 'data'], text=True).strip())
        project = ROOT / 'scripts/ios/smoke-qa'
        run('xcodegen', 'generate', '--spec', str(project / 'project.yml'), '--project', str(project))
        with (destination / 'xctest.log').open('w') as log:
            run('xcodebuild', 'test', '-project', str(project / 'SmokeOracle.xcodeproj'),
                '-scheme', 'SmokeOracle', '-destination', 'id=' + device,
                '-derivedDataPath', str(ROOT / 'target/ios-smoke/build'),
                '-resultBundlePath', str(destination / 'result.xcresult'), 'CODE_SIGNING_ALLOWED=NO',
                '-only-testing:SmokeOracle/SmokeQA/testSH1152Preflight', stdout=log, stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, flush=True)
        raise
    finally:
        if container:
            for name in ('p0-smoke.jsonl', 'p4-peer.jsonl', 'sh1-152-sdk-probe.log'):
                source = container / 'Documents' / name
                if source.is_file():
                    shutil.copyfile(source, destination / name)
        subprocess.run(['xcrun', 'simctl', 'shutdown', device], check=False)
        run('xcrun', 'simctl', 'delete', device)
        print('PREFLIGHT EVIDENCE', destination, flush=True)


if __name__ == '__main__':
    main()
