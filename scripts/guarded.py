#!/usr/bin/env python3
import os, signal, subprocess, sys, time, threading
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent / "target"
ROOT.mkdir(exist_ok=True)

def probe():
    v = os.statvfs('/System/Volumes/Data')
    free = v.f_bavail * v.f_frsize
    with (ROOT / 'disk.log').open('a') as log:
        log.write(f'disk {time.strftime("%Y-%m-%dT%H:%M:%S%z")} free_bytes={free} free_GiB={free/2**30:.3f}\n')
    return free >= 6 * 2**30

def stop():
    (ROOT / 'disk-guard-fired').write_text('Free disk fell below 6 GiB; stop all work.\n')
    with (ROOT / 'disk.log').open('a') as log:
        for target in sorted((Path.home() / 'ManagedProjects').glob('herdr-gpui*/target')):
            subprocess.run(['du', '-sh', str(target)], stdout=log, stderr=log)
    return 86

if not probe():
    sys.exit(stop())
with (ROOT / 'disk.log').open('a') as log:
    log.write('COMMAND '+repr([('PATH=<rustup proxies first>' if a.startswith('PATH=') else a) for a in sys.argv[1:]])+'\n'); log.flush()
    p = subprocess.Popen(sys.argv[1:], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
    def forward():
        for line in p.stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
            log.write(line)
            log.flush()
    output = threading.Thread(target=forward)
    output.start()
    while p.poll() is None:
        time.sleep(5)
        if not probe():
            os.killpg(p.pid, signal.SIGTERM)
            try: p.wait(timeout=10)
            except subprocess.TimeoutExpired: os.killpg(p.pid, signal.SIGKILL); p.wait()
            sys.exit(stop())
    output.join()
    log.write(f'EXIT {p.returncode}\n')
    sys.exit(p.returncode)
