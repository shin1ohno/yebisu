#!/usr/bin/env python3
"""Run one process group with a continuously observed 6 GiB disk floor."""
import codecs
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent / "target"
ROOT.mkdir(exist_ok=True)


def probe():
    disk = os.statvfs(ROOT)
    free = disk.f_bavail * disk.f_frsize
    with (ROOT / 'disk.log').open('a') as log:
        log.write(f'disk {time.strftime("%Y-%m-%dT%H:%M:%S%z")} free_bytes={free} free_GiB={free / 2**30:.3f}\n')
    return free >= 6 * 2**30


def stop():
    (ROOT / 'disk-guard-fired').write_text('Free disk fell below 6 GiB; stop all work.\n')
    with (ROOT / 'disk.log').open('a') as log:
        for target in sorted((Path.home() / 'ManagedProjects').glob('herdr-gpui*/target')):
            subprocess.run(['du', '-sh', str(target)], stdout=log, stderr=log)
    return 86


def group_exists(group):
    try:
        os.killpg(group, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        # Darwin may report EPERM for a group containing only exiting zombies.
        rows = subprocess.check_output(['ps', '-axo', 'pgid=,stat='], text=True)
        return any(int(parts[0]) == group and not parts[1].startswith('Z')
                   for line in rows.splitlines() if len(parts := line.split()) == 2)


def signal_group(group, value):
    try:
        os.killpg(group, value)
    except ProcessLookupError:
        pass
    except PermissionError:
        if group_exists(group):
            raise


def terminate_group(process, log):
    group = process.pid
    signal_group(group, signal.SIGTERM)
    deadline = time.monotonic() + 10
    while group_exists(group) and time.monotonic() < deadline:
        process.poll()  # Reap the leader, independently of remaining group members.
        time.sleep(0.1)
    if group_exists(group):
        log.write(f'GUARD SIGKILL remaining process group {group}\n')
        log.flush()
        signal_group(group, signal.SIGKILL)
    process.wait(timeout=2)


def main(arguments):
    if not arguments:
        print('usage: guarded.py COMMAND [ARGS...]', file=sys.stderr)
        return 2
    if not probe():
        return stop()
    with (ROOT / 'disk.log').open('a') as log:
        log.write('COMMAND ' + repr(arguments) + '\n')
        log.flush()
        process = subprocess.Popen(arguments, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        descriptor = process.stdout.fileno()
        os.set_blocking(descriptor, False)
        decoder = codecs.getincrementaldecoder('utf-8')(errors='replace')
        selector = selectors.DefaultSelector()
        selector.register(descriptor, selectors.EVENT_READ)
        next_probe = time.monotonic() + 5
        eof = False
        try:
            while process.poll() is None or not eof:
                for _, _ in selector.select(timeout=0.1):
                    chunk = os.read(descriptor, 65536)
                    if not chunk:
                        eof = True
                        selector.unregister(descriptor)
                        break
                    text = decoder.decode(chunk)
                    sys.stdout.write(text)
                    sys.stdout.flush()
                    log.write(text)
                    log.flush()
                if time.monotonic() >= next_probe:
                    if not probe():
                        terminate_group(process, log)
                        # No blocking reader/join: a retained pipe cannot hold the guard open.
                        return stop()
                    next_probe = time.monotonic() + 5
            tail = decoder.decode(b'', final=True)
            sys.stdout.write(tail)
            log.write(tail)
            log.write(f'EXIT {process.returncode}\n')
            return process.returncode
        finally:
            selector.close()
            process.stdout.close()


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
