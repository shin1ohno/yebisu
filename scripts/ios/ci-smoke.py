#!/usr/bin/env python3
"""Run the existing smoke check on a disposable, available iPad Simulator."""
import json
import os
from pathlib import Path
import subprocess
import sys


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


def main(p9=False):
    listing = subprocess.check_output(['xcrun', 'simctl', 'list', '--json', 'devices', 'available'], text=True)
    device_type, runtime = select_template(json.loads(listing)['devices'])
    device = subprocess.check_output(
        ['xcrun', 'simctl', 'create', 'yebisu CI smoke', device_type, runtime], text=True,
    ).strip()
    print(f'CI SMOKE simulator={device} type={device_type} runtime={runtime}', flush=True)
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
    finally:
        # Delete only our disposable device, including after a failed smoke check.
        # shutdown may fail if the build failed before booting; delete must succeed.
        shutdown = subprocess.run(['xcrun', 'simctl', 'shutdown', device], check=False)
        subprocess.run(['xcrun', 'simctl', 'delete', device], check=True)
        print(f'CI SMOKE cleanup simulator={device} shutdown_exit={shutdown.returncode} deleted', flush=True)


if __name__ == '__main__':
    main(p9="--p9" in sys.argv)
