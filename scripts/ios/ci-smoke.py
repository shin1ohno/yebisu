#!/usr/bin/env python3
"""Run the existing smoke check on a disposable, available iPad Simulator.

On the xcode-27 image a freshly booted Simulator blocks its first install for
several minutes after `simctl bootstatus` returns (issue #235: 3.5-8.9 min,
against 1.0-2.3 min on macos-26), and later installs on the same device take
seconds. So in GitHub Actions with --p9, a passing run hands the warm
Simulator to the next step, p12-ci.py, instead of letting P12 boot a second
one; p12-ci.py deletes it, and any failure deletes it here. Booting before the
build was tried and dropped: in run 45856917743 the build and the readiness
wait grew by about the time the install saved.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
# The UDID that a passing `ci-smoke.py --p9` leaves for p12-ci.py in the same job.
HANDOVER = ROOT / 'target' / 'ci-smoke-simulator.txt'


def select_template(devices):
    candidates = []
    for runtime, entries in devices.items():
        if '.iOS-' not in runtime:
            continue
        suffix = runtime.split('.iOS-', 1)[1]
        if not all(part.isascii() and part.isdecimal() for part in suffix.split('-')):
            print(f'CI SMOKE skipping runtime={runtime}: non-numeric iOS version', flush=True)
            continue
        version = tuple(int(part) for part in suffix.split('-'))
        for entry in entries:
            if entry['isAvailable'] and entry['name'].startswith('iPad Pro 13-inch'):
                candidates.append((version, runtime, entry['deviceTypeIdentifier']))
    if not candidates:
        raise RuntimeError('No available iPad Pro 13-inch Simulator; inspect xcrun simctl list devices available')
    _, runtime, device_type = max(candidates)
    return device_type, runtime


def hands_over(p9):
    """Only the hosted job's P0+P9 step has a P12 step after it to take the Simulator."""
    return p9 and os.environ.get('GITHUB_ACTIONS') == 'true'


def main(p9=False):
    listing = subprocess.check_output(['xcrun', 'simctl', 'list', '--json', 'devices', 'available'], text=True)
    device_type, runtime = select_template(json.loads(listing)['devices'])
    device = subprocess.check_output(
        ['xcrun', 'simctl', 'create', 'yebisu CI smoke', device_type, runtime], text=True,
    ).strip()
    print(f'CI SMOKE simulator={device} type={device_type} runtime={runtime}', flush=True)
    passed = False
    try:
        # smoke-check.py builds, boots, installs, launches and calls smoke_oracle.
        # It uses ad-hoc simulator signing and the local P0 screen, with no peer.
        subprocess.run(
            [sys.executable, str(Path(__file__).with_name('smoke-check.py'))],
            env={**os.environ, 'HERDR_IOS_SIMULATOR': device}, check=True,
        )
        if p9:
            subprocess.run(
                [sys.executable, str(Path(__file__).with_name('p9-check.py'))],
                env={**os.environ, 'HERDR_IOS_SIMULATOR': device}, check=True,
            )
        passed = True
    finally:
        if passed and hands_over(p9):
            HANDOVER.parent.mkdir(parents=True, exist_ok=True)
            HANDOVER.write_text(device + '\n')
            print(f'CI SMOKE handover simulator={device} to p12-ci.py ({HANDOVER.relative_to(ROOT)})', flush=True)
        else:
            # Delete only our disposable device, including after a failed smoke check.
            # shutdown may fail if the build failed before booting; delete must succeed.
            shutdown = subprocess.run(['xcrun', 'simctl', 'shutdown', device], check=False)
            subprocess.run(['xcrun', 'simctl', 'delete', device], check=True)
            print(f'CI SMOKE cleanup simulator={device} shutdown_exit={shutdown.returncode} deleted', flush=True)


if __name__ == '__main__':
    main(p9="--p9" in sys.argv)
