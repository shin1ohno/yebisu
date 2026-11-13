"""Cooperative owner and pre-write admission for the shared Simulator chain.

No protection against a hostile same-UID process or malicious Cargo/build scripts.
Admission is read-only. A rejected caller cannot create even an owner/log file.
"""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import uuid


SCHEMA = 'cargo-provenance-v2'
PROFILE_FIELDS = {'opt_level', 'debuginfo', 'debug_assertions', 'overflow_checks', 'test'}


def load_helper():
    spec = importlib.util.spec_from_file_location('bundle_provenance', Path(__file__).with_name('bundle-provenance.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def canonical(path, *, exists=True):
    path = Path(path)
    if not path.is_absolute() or '..' in path.parts or str(path) != str(path.resolve(strict=exists)):
        raise ValueError('requires canonical absolute path without symlinks: ' + str(path))
    return path


def child(path, root, *, exists=True):
    path, root = canonical(path, exists=exists), canonical(root)
    if path == root or not path.is_relative_to(root):
        raise ValueError('path outside dedicated owned root: ' + str(path))
    return path


def configs(source, environment):
    """Snapshot both present and absent Cargo config candidates (never credentials)."""
    candidates = set()
    for parent in (source, *source.parents):
        candidates.update(parent / '.cargo' / name for name in ('config', 'config.toml'))
    cargo_home = Path(environment.get('CARGO_HOME', str(Path.home() / '.cargo'))).resolve()
    candidates.update(cargo_home / name for name in ('config', 'config.toml'))
    candidates.update(source / name for name in ('rust-toolchain', 'rust-toolchain.toml'))
    result = {}
    import tomllib
    for path in sorted(candidates):
        if path.is_symlink():
            raise ValueError('symlink Cargo configuration unsupported')
        if path.exists():
            if not path.is_file():
                raise ValueError('non-file Cargo configuration')
            raw = path.read_bytes()
            result[str(path)] = hashlib.sha256(raw).hexdigest()
            if path.name in ('config', 'config.toml'):
                data = tomllib.loads(raw.decode())
                if 'include' in data or any(k in data.get('build', {}) for k in
                                           ('rustc-wrapper', 'rustc-workspace-wrapper', 'build-dir')):
                    raise ValueError('unsupported Cargo include/wrapper/build-dir configuration')
                if any(k in data.get('env', {}) for k in ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
                                                        'CARGO_TARGET_DIR', 'CARGO_BUILD_BUILD_DIR')):
                    raise ValueError('Cargo config may override provenance ownership')
        else:
            result[str(path)] = None
    return result


def validate_contract(expected):
    if not isinstance(expected, dict) or expected.get('schema') != SCHEMA:
        raise ValueError('explicit typed provenance contract required')
    keys = {'schema', 'head', 'run_id', 'package', 'binary', 'target', 'features',
            'no_default_features', 'profile_name', 'profile', 'sdk'}
    if set(expected) != keys:
        raise ValueError('unknown/missing provenance contract field')
    if not re.fullmatch('[0-9a-f]{40}', expected['head']):
        raise ValueError('full expected source head required')
    if str(uuid.UUID(expected['run_id'])) != expected['run_id']:
        raise ValueError('canonical run UUID required')
    if (expected['package'], expected['binary'], expected['target']) != ('yebisu', 'yebisu', 'aarch64-apple-ios-sim'):
        raise ValueError('provenance supports the explicit yebisu Simulator target only')
    if (not isinstance(expected['features'], list) or
            expected['features'] != sorted(set(expected['features'])) or
            any(not isinstance(f, str) or not re.fullmatch(r'[A-Za-z0-9_/-]+', f) for f in expected['features']) or
            type(expected['no_default_features']) is not bool):
        raise ValueError('invalid requested features')
    p = expected['profile']
    if (expected['profile_name'] not in ('dev', 'release') or not isinstance(p, dict) or set(p) != PROFILE_FIELDS or
            p['opt_level'] not in ('0', '1', '2', '3', 's', 'z') or type(p['debuginfo']) is not int or
            p['debuginfo'] not in (0, 1, 2) or any(type(p[f]) is not bool for f in
                                                 ('debug_assertions', 'overflow_checks', 'test')) or p['test']):
        raise ValueError('invalid explicit compiler profile')
    sdk = expected['sdk']
    if not isinstance(sdk, list) or not sdk or len({s['name'] for s in sdk}) != len(sdk):
        raise ValueError('explicit unique expected SDK packages required')
    for s in sdk:
        if (set(s) != {'name', 'url', 'revision'} or not re.fullmatch('[0-9a-f]{40}', s['revision']) or
                not s['url'].startswith('https://') or not s['name']):
            raise ValueError('invalid expected SDK identity')


def bundle_contract(bundle_args, expected):
    parsed = dict(package='yebisu', binary=None, target='aarch64-apple-ios-sim',
                  features=[], no_default_features=False, profile_name='dev')
    seen = set()
    it = iter(bundle_args)
    for arg in it:
        if arg in seen:
            raise ValueError('duplicate bundle contract argument')
        seen.add(arg)
        if arg in ('--package', '--bin', '--target', '--features'):
            value = next(it, None)
            if value is None:
                raise ValueError('missing bundle argument')
            key = {'--package': 'package', '--bin': 'binary', '--target': 'target', '--features': 'features'}[arg]
            parsed[key] = sorted(value.split(',')) if key == 'features' else value
        elif arg == '--release':
            parsed['profile_name'] = 'release'
        elif arg == '--no-default-features':
            parsed['no_default_features'] = True
        else:
            raise ValueError('unsupported provenance bundle argument: ' + arg)
    if any(parsed[k] != expected[k] for k in parsed):
        raise ValueError('bundle arguments differ from expected contract')


def admit(directory, source, device, bundle_args, environment, expected):
    """Read-only first-write guard; no mkdir/open-for-write/logging in this function."""
    validate_contract(expected)
    uuid.UUID(device)
    root = canonical(environment['YEBISU_OWN_ROOT'])
    directory, source = child(directory, root), child(source, root)
    target = child(environment['CARGO_TARGET_DIR'], root)
    if not all(p.is_dir() for p in (root, directory, source, target)):
        raise ValueError('ownership paths must be existing directories')
    if any(directory.is_relative_to(p) or p.is_relative_to(directory) for p in (source, target)):
        raise ValueError('run directory overlaps source/target')
    if target == source or (target.is_relative_to(source) and not target.is_relative_to(source / '.git')):
        raise ValueError('target overlaps tracked source')
    if source.is_relative_to(target):
        raise ValueError('target contains source')
    if any(directory.iterdir()):
        raise ValueError('run directory must be fresh and empty')
    if any(p.stat().st_uid != os.getuid() for p in (root, source, target, directory)):
        raise ValueError('ownership uid mismatch')
    if environment.get('YEBISU_EXPECTED_HEAD') != expected['head']:
        raise ValueError('expected head environment mismatch')
    if environment.get('YEBISU_BUNDLE_PROVENANCE_DIR', str(directory)) != str(directory):
        raise ValueError('run directory environment mismatch')
    if any(environment.get(k) for k in ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_BUILD_DIR',
                                       'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER')):
        raise ValueError('nested compiler wrappers/build-dir unsupported')
    def git(*args):
        return subprocess.check_output(['git', '-C', str(source), *args], text=True, env=dict(environment, GIT_OPTIONAL_LOCKS='0')).strip()
    if git('rev-parse', '--show-toplevel') != str(source) or git('rev-parse', 'HEAD') != expected['head']:
        raise ValueError('unexpected source checkout/head')
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('tracked source is dirty')
    bundle_contract(bundle_args, expected)
    config = configs(source, environment)
    return dict(schema=SCHEMA, expected=expected, own_root=str(root), source=str(source),
                target=str(target), directory=str(directory), app=str(directory / 'yebisu.app'),
                bundle_args=list(bundle_args), config=config)


def require_owner(directory):
    directory = canonical(directory)
    owner_path, lock_path = directory / 'owner.json', directory / 'owner.lock'
    if any(p.is_symlink() or not p.is_file() for p in (owner_path, lock_path)):
        raise ValueError('missing regular owner files')
    owner = json.loads(owner_path.read_text())
    if owner['token'] != os.environ.get('YEBISU_PROVENANCE_TOKEN') or owner['directory'] != str(directory):
        raise ValueError('missing run owner token/directory')
    child(directory, owner['own_root'])
    with lock_path.open('r') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return owner
        raise ValueError('run owner lock is not held')


def build_install(directory, source, device, bundle_args, environment, *, expected):
    owner = admit(directory, source, device, bundle_args, environment, expected)
    directory, source = Path(owner['directory']), Path(owner['source'])
    with (directory / 'owner.lock').open('x') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        token = uuid.uuid4().hex
        owner.update(token=token, pid=os.getpid(), device=device)
        with (directory / 'owner.json').open('x') as out:
            json.dump(owner, out)
        env = dict(environment, YEBISU_PROVENANCE_TOKEN=token, PYTHONDONTWRITEBYTECODE='1',
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
        if app != owner['app']:
            raise ValueError('bundle returned another app path')
        run('python3', '-I', '-B', helper, 'preinstall', '--directory', str(directory), '--app', app)
        run('xcrun', 'simctl', 'install', device, app)
        installed = run('xcrun', 'simctl', 'get_app_container', device, 'com.shin1ohno.yebisu', 'app')
        run('python3', '-I', '-B', helper, 'installed', '--directory', str(directory),
            '--app', installed, '--device', device)
        return dict(app=app, installed=installed, device=device, run_id=expected['run_id'],
                    binding=str(directory / 'binding.json'))
