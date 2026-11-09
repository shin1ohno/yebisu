#!/usr/bin/env python3
"""Build/run the smoke screen and validate actual paint/input receipts."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import uuid
import smoke_oracle


def run(*command, **kwargs):
    print('SMOKE COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wait', default='5')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    destination = root / 'target/ios-smoke' / (time.strftime('%Y%m%d-%H%M%S')+'-'+uuid.uuid4().hex[:6])
    destination.mkdir(parents=True)
    app = run('bash', 'scripts/ios/bundle.sh', '--package', 'yebisu', '--bin', 'yebisu', capture_output=True)
    print(app.stderr, flush=True)
    app_path = app.stdout.strip().splitlines()[-1]
    environment = dict(os.environ, SIMCTL_CHILD_YEBISU_SMOKE_OBSERVE='1')
    run('bash', 'scripts/ios/sim.sh', '--wait', args.wait, '--out', str(destination/'launch'), app_path, env=environment)
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True)
    device = resolved.split()[0]
    spec = root/'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec/'project.yml'), '--project', str(spec))
    result = destination/'result.xcresult'
    run('xcodebuild', 'test', '-project', str(spec/'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle', '-destination', 'id='+device, '-derivedDataPath', str(root/'target/ios-smoke/build'), '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO')
    export = destination/'attachments'
    run('xcrun', 'xcresulttool', 'export', 'attachments', '--path', str(result), '--output-path', str(export))
    stages = []
    for test in json.loads((export/'manifest.json').read_text()):
        for attachment in test['attachments']:
            stage = attachment['suggestedHumanReadableName'].split('_0_')[0]
            if stage not in smoke_oracle.EXPECTED:
                continue
            name = stage+'.png'
            shutil.copyfile(export/attachment['exportedFileName'], destination/name)
            stages.append({'stage':stage, 'file':name, 'time':attachment['timestamp']})
    (destination/'stages.json').write_text(json.dumps(stages, indent=2)+'\n')
    container = subprocess.check_output(['xcrun','simctl','get_app_container',device,'com.shin1ohno.yebisu','data'], text=True).strip()
    shutil.copyfile(Path(container)/'Documents/p0-smoke.jsonl', destination/'app.jsonl')
    smoke_oracle.validate(destination)
    (root/'target/ios-smoke/latest.json').write_text(json.dumps({'directory':str(destination)})+'\n')
    print('SMOKE RECEIVING EVIDENCE', destination, flush=True)


if __name__ == '__main__':
    main()
