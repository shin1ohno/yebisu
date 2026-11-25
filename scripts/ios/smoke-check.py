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
import importlib.util

_ready_spec = importlib.util.spec_from_file_location("sim_ready", Path(__file__).with_name("sim_ready.py"))
sim_ready = importlib.util.module_from_spec(_ready_spec)
_ready_spec.loader.exec_module(sim_ready)
_budget_spec = importlib.util.spec_from_file_location("resend_budget", Path(__file__).with_name("resend_budget.py"))
resend_budget = importlib.util.module_from_spec(_budget_spec)
_budget_spec.loader.exec_module(resend_budget)


def run(*command, **kwargs):
    print('SMOKE COMMAND', list(command), flush=True)
    if command[:2] == ("xcodebuild", "test"):
        # Issue #142 stage 4: streamed as before, and kept for the job's resend budget.
        # Class D: once more when xcodebuild found no device and ran no test (sim_ready.run_test).
        result = sim_ready.run_test(list(command), lambda: subprocess.CompletedProcess(list(command), *resend_budget.run_tee(list(command), **kwargs)))
        resend_budget.record(Path(__file__).stem, result.stdout)
        if result.returncode:
            raise subprocess.CalledProcessError(result.returncode, command, result.stdout)
        return result
    return subprocess.run(command, check=True, text=True, **kwargs)


def terminate(device):
    """Stops the app so none of its files is read while it may still write (UI test redesign S2:
    files are read only after terminate; S6 for P0). Exit 3 is simctl's 'nothing to terminate'."""
    result = subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    print('P0 terminate before reading the app\'s files: exit', result.returncode, (result.stderr or '').strip(), flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError('could not terminate the app before reading its files (simctl exit %d)' % result.returncode)


def idle_report(events):
    """The animations-running records (animation_qa.rs: an animation that stayed 10 s), printed by
    name: they say what kept the app from idling when XCTest waited for it (class C-anim, S5
    follow-up S6-2). None is the normal case."""
    stuck = [e for e in events if e.get('kind') == 'animations-running']
    print('P0 animations-running records:', len(stuck), json.dumps(stuck)[:2000], flush=True)
    return stuck


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
    # YEBISU_QA_AX (UI test redesign S6): testInputAndLifecycleReceipts waits on qa.state, not OCR.
    environment = dict(os.environ, SIMCTL_CHILD_YEBISU_SMOKE_OBSERVE='1', SIMCTL_CHILD_YEBISU_QA_AX='1')
    run('bash', 'scripts/ios/sim.sh', '--wait', args.wait, '--out', str(destination/'launch'), app_path, env=environment)
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True)
    device = resolved.split()[0]
    spec = root/'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec/'project.yml'), '--project', str(spec))
    # Class D (issue #142 stage 4): a new Simulator can be missing from xcodebuild's list for a while.
    sim_ready.wait_for_destination(spec / "SmokeOracle.xcodeproj", "SmokeOracle", device)
    result = destination/'result.xcresult'
    container = subprocess.check_output(['xcrun','simctl','get_app_container',device,'com.shin1ohno.yebisu','data'], text=True).strip()
    try:
        run('xcodebuild', 'test', '-project', str(spec/'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle', '-destination', 'id='+device, '-derivedDataPath', str(root/'target/ios-smoke/build'), '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO', '-only-testing:SmokeOracle/SmokeQA/testInputAndLifecycleReceipts')
    finally:
        terminate(device)
        # The app is stopped before its receipts are copied, also when XCTest failed, and the
        # copy is kept as evidence of a failed run.
        receipts = Path(container)/'Documents/p0-smoke.jsonl'
        if receipts.is_file():
            shutil.copyfile(receipts, destination/'app.jsonl')
            try:
                events = smoke_oracle.live_jsonl.read_records(destination/'app.jsonl')
                idle_report(events)
                # Issue #142 stage 4 (classes Q and S): main-thread stalls, reported, not judged.
                gaps = smoke_oracle.live_jsonl.heartbeat_gaps(events)
                (destination/'heartbeat-gaps.json').write_text(json.dumps(gaps, indent=1)+'\n')
                print('P0 heartbeat gaps:', json.dumps(gaps), flush=True)
            except (OSError, ValueError) as error:  # never hide XCTest's own failure (PR 84 review N3)
                print('P0 animations-running records unreadable:', error, flush=True)
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
    smoke_oracle.validate(destination)
    (root/'target/ios-smoke/latest.json').write_text(json.dumps({'directory':str(destination)})+'\n')
    print('SMOKE RECEIVING EVIDENCE', destination, flush=True)


if __name__ == '__main__':
    main()
