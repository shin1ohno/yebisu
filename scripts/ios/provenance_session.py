"""Exclusive cooperative owner for one bundle/sign/install evidence chain.

This is not isolation from hostile same-UID processes. All run participants must
honor ownership. No shell expansion, no device signing and no key acquisition.
"""
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import uuid


def load_helper():
    spec = importlib.util.spec_from_file_location('bundle_provenance', Path(__file__).with_name('bundle-provenance.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def require_owner(directory):
    owner = json.loads((directory / 'owner.json').read_text())
    if owner['token'] != os.environ.get('YEBISU_PROVENANCE_TOKEN'):
        raise ValueError('missing run owner token')
    with (directory / 'owner.lock').open('r') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return
        raise ValueError('run owner lock is not held')


def build_install(directory, source, device, bundle_args, environment):
    directory, source = Path(directory).resolve(strict=True), Path(source).resolve(strict=True)
    uuid.UUID(device)
    # Exclusive creation prevents run reuse, including abandoned partial runs.
    with (directory / 'owner.lock').open('x') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        token = uuid.uuid4().hex
        with (directory / 'owner.json').open('x') as out:
            json.dump(dict(token=token, pid=os.getpid(), device=device), out)
        env = dict(environment, YEBISU_PROVENANCE_TOKEN=token,
                   YEBISU_BUNDLE_PROVENANCE_DIR=str(directory))
        def run(*args):
            result = subprocess.run(args, cwd=source, env=env, text=True, capture_output=True)
            with (directory / 'session.log').open('a') as log:
                log.write(json.dumps(list(args)) + '\n' + result.stdout + result.stderr)
            result.check_returncode()
            return result.stdout.strip()
        helper = str(Path(__file__).with_name('bundle-provenance.py'))
        output = run('bash', 'scripts/ios/bundle.sh', *bundle_args)
        app = output.splitlines()[-1]
        run('python3', '-I', helper, 'preinstall', '--directory', str(directory), '--app', app)
        run('xcrun', 'simctl', 'install', device, app)
        installed = run('xcrun', 'simctl', 'get_app_container', device, 'com.shin1ohno.yebisu', 'app')
        run('python3', '-I', helper, 'installed', '--directory', str(directory),
            '--app', installed, '--device', device)
        return dict(app=app, installed=installed, device=device, binding=str(directory / 'binding.json'))
