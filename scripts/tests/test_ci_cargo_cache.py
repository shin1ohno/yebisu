"""ci.yml's cargo cache: every job that compiles Rust keeps only its own cargo outputs, keyed per job.

problems() is the check; the negative controls below show that each rule it states fails a workflow that
breaks it, so a later edit cannot quietly widen what is cached or share one job's entry with another."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CI = (ROOT / '.github/workflows/ci.yml').read_text()
TESTFLIGHT = (ROOT / '.github/workflows/testflight.yml').read_text()

RUST_JOBS = ['edit-menu', 'simulator-smoke', 'check', 'layout-smoke']
TOOLCHAIN = 'python3 scripts/guarded.py rustup show active-toolchain'
FETCH = 'just guarded cargo fetch --locked'
# Downloaded sources, and the compiled dependencies of each target directory a job's cargo commands write.
# Everything else under target/ (the .app bundle.sh writes beside the executable, XCTest and Simulator
# output, evidence) stays out.
ALLOWED_PATH = re.compile(r'~/\.cargo/(registry/index|registry/cache|git/db)/'
                          r'|target/(debug|aarch64-apple-ios/debug|aarch64-apple-ios-sim/debug)/(deps|build|\.fingerprint)/')
FORBIDDEN = ('.app', '.ipa', '.p12', '.p8', '.mobileprovision', 'CoreSimulator', 'Library/', 'target/ios-', 'evidence',
             'secrets.', 'Keychain')
KEY_PARTS = ('${{ runner.os }}', '${{ github.job }}', "hashFiles('rust-toolchain.toml')",
             '${{ steps.cargo-cache-key.outputs.xcode }}', "hashFiles('Cargo.lock')",
             "hashFiles('Cargo.toml', 'crates/*/Cargo.toml', '.cargo/config.toml')")
# The parts restore-keys must keep, so a miss falls back only to an entry of the same job and toolchain.
PREFIX_PARTS = KEY_PARTS[:4]


def jobs(workflow):
    found = {}
    for match in re.finditer(r'^  ([a-z][a-z-]*):\n(.*?)(?=^  [a-z][a-z-]*:\n|\Z)',
                             workflow.split('\njobs:\n', 1)[1], re.S | re.M):
        found[match.group(1)] = match.group(2)
    return found


def steps(job):
    body = re.sub(r'^\s*#.*\n', '', job.split('\n    steps:\n', 1)[1], flags=re.M)
    return [{'text': part, 'name': part.split('\n', 1)[0]} for part in re.split(r'^      - ', body, flags=re.M)[1:]]


def position(found, needle):
    hits = [i for i, step in enumerate(found) if needle in step['text']]
    return hits[0] if len(hits) == 1 else None


def field(step, name):
    match = re.search(rf'^          {name}: (.*)$', step, re.M)
    return match.group(1) if match else None


def block(step, name):
    match = re.search(rf'^          {name}: \|\n((?:            .*\n)+)', step, re.M)
    return [line.strip() for line in match.group(1).splitlines()] if match else []


def problems(workflow, testflight):
    found = []
    every = jobs(workflow)
    compiling = sorted(name for name, job in every.items() if TOOLCHAIN in job)
    if compiling != sorted(RUST_JOBS):
        found.append(f'jobs that install Rust are {compiling}, expected {sorted(RUST_JOBS)}')
    cache_steps = {}
    for name in RUST_JOBS:
        job = every.get(name, '')
        # macOS images: the macos-* labels and xcode-27 (macOS 27 with Xcode 27).
        if not re.search(r'^    runs-on: (macos-\S+|xcode-27)$', job, re.M):
            found.append(f'{name}: not a macOS job')
            continue
        listed = steps(job)
        order = [position(listed, TOOLCHAIN), position(listed, 'id: cargo-cache-key'),
                 position(listed, 'uses: actions/cache@'), position(listed, FETCH)]
        if None in order or order != sorted(order):
            found.append(f'{name}: expected guarded toolchain, cache key, cache, then guarded fetch; got {order}')
            continue
        key_step, cache = listed[order[1]]['text'], listed[order[2]]['text']
        cache_steps[name] = key_step + cache
        if '|| echo unknown' not in key_step:
            found.append(f'{name}: the cache key step fails when xcodebuild fails')
        if not re.search(r'uses: actions/cache@[0-9a-f]{40} # v\d+\.\d+\.\d+\n', cache):
            found.append(f'{name}: actions/cache is not pinned to a full commit SHA')
        for flag in ('fail-on-cache-miss', 'lookup-only', 'continue-on-error', 'secrets.', 'enableCrossOsArchive'):
            if flag in cache:
                found.append(f'{name}: the cache step sets {flag}')
        paths = block(cache, 'path')
        for path in paths:
            if not ALLOWED_PATH.fullmatch(path) or any(word in path for word in FORBIDDEN):
                found.append(f'{name}: caches {path}')
        if '~/.cargo/registry/cache/' not in paths or '~/.cargo/git/db/' not in paths:
            found.append(f'{name}: does not cache the cargo registry and git sources')
        key = field(cache, 'key') or ''
        for part in KEY_PARTS:
            if part not in key:
                found.append(f'{name}: key lacks {part}')
        fallback = block(cache, 'restore-keys')
        if len(fallback) != 1 or not key.startswith(fallback[0]) or not fallback[0].endswith('-'):
            found.append(f'{name}: restore-keys must be one prefix of the key, got {fallback}')
        elif any(part not in fallback[0] for part in PREFIX_PARTS) or 'Cargo.lock' in fallback[0]:
            found.append(f'{name}: restore-keys do not stay within the job, toolchain and Xcode')
    if len(set(cache_steps.values())) > 1:
        found.append('the cache steps differ between jobs; github.job is what keeps their entries apart')
    if 'actions/cache' in testflight or 'cargo-cache' in testflight:
        found.append('testflight.yml uses a cache')
    return found


class CargoCacheTests(unittest.TestCase):
    def test_the_workflows_pass(self):
        self.assertEqual(problems(CI, TESTFLIGHT), [])

    def test_the_longest_key_fits_and_has_no_comma(self):
        cache = steps(jobs(CI)['simulator-smoke'])
        key = field(next(s['text'] for s in cache if 'uses: actions/cache@' in s['text']), 'key')
        expanded = re.sub(r'\$\{\{[^}]*\}\}', lambda m: 'simulator-smoke' if 'github.job' in m.group(0) else 'f' * 64, key)
        self.assertLessEqual(len(expanded), 512)  # actions/cache rejects a longer key and fails the step
        self.assertNotIn(',', expanded)

    def test_the_key_step_writes_an_xcode_digest_even_without_xcodebuild(self):
        key_step = next(s['text'] for s in steps(jobs(CI)['check']) if 'id: cargo-cache-key' in s['text'])
        command = re.search(r'^        run: (.*)$', key_step, re.M).group(1)
        with tempfile.TemporaryDirectory() as directory:
            stub = Path(directory) / 'xcodebuild'
            output = Path(directory) / 'output'
            for body, label in (('exit 1', 'missing'), ('echo "Xcode 26.1"; echo "Build version 17B55"', 'present')):
                with self.subTest(xcodebuild=label):
                    stub.write_text(f'#!/bin/sh\n{body}\n')
                    stub.chmod(0o755)
                    output.write_text('')
                    env = {**os.environ, 'PATH': f'{directory}:{os.environ["PATH"]}', 'GITHUB_OUTPUT': str(output)}
                    subprocess.run(['bash', '--noprofile', '--norc', '-eo', 'pipefail', '-c', command], env=env, check=True)
                    self.assertRegex(output.read_text(), r'^xcode=[0-9a-f]{16}\n$')


class NegativeControls(unittest.TestCase):
    def assert_caught(self, workflow, expected, testflight=TESTFLIGHT):
        self.assertNotEqual(workflow, CI, 'the mutation did not apply')
        self.assertTrue(any(expected in problem for problem in problems(workflow, testflight)),
                        problems(workflow, testflight))

    def test_a_whole_target_directory_is_caught(self):
        self.assert_caught(CI.replace('target/aarch64-apple-ios-sim/debug/deps/', 'target/aarch64-apple-ios-sim/debug/'),
                           'caches target/aarch64-apple-ios-sim/debug/')

    def test_a_simulator_directory_is_caught(self):
        self.assert_caught(CI.replace('            ~/.cargo/git/db/\n', '            ~/.cargo/git/db/\n            target/ios-smoke/\n'),
                           'caches target/ios-smoke/')

    def test_a_tag_instead_of_a_sha_is_caught(self):
        self.assert_caught(re.sub(r'actions/cache@[0-9a-f]{40}', 'actions/cache@v6', CI), 'not pinned')

    def test_a_fallback_shared_between_jobs_is_caught(self):
        self.assert_caught(CI.replace("cargo-v1-${{ runner.os }}-${{ runner.arch }}-${{ github.job }}-${{ hashFiles('rust-toolchain.toml') }}-xcode-${{ steps.cargo-cache-key.outputs.xcode }}-\n",
                                      'cargo-v1-${{ runner.os }}-\n'), 'restore-keys')

    def test_a_key_without_the_lockfile_is_caught(self):
        self.assert_caught(CI.replace("-${{ hashFiles('Cargo.lock') }}", ''), "key lacks hashFiles('Cargo.lock')")

    def test_a_cache_restored_after_the_fetch_is_caught(self):
        job = jobs(CI)['layout-smoke']
        fetch = f'      - name: Fetch\n        run: {FETCH}\n'
        cache = re.search(r'      - name: Cargo cache key\n.*?(?=      - name: Fetch\n)', job, re.S).group(0)
        moved = job.replace(cache, '').replace(fetch, fetch + cache)
        self.assert_caught(CI.replace(job, moved), 'layout-smoke: expected guarded toolchain')

    def test_a_job_failing_on_a_miss_is_caught(self):
        self.assert_caught(CI.replace('          restore-keys: |\n', '          fail-on-cache-miss: true\n          restore-keys: |\n', 1),
                           'fail-on-cache-miss')

    def test_a_job_left_without_the_cache_is_caught(self):
        job = jobs(CI)['edit-menu']
        cache = re.search(r'      - name: Cargo cache key\n.*?(?=      - name: Fetch\n)', job, re.S).group(0)
        self.assert_caught(CI.replace(job, job.replace(cache, '')), 'edit-menu: expected guarded toolchain')

    def test_a_cache_in_testflight_is_caught(self):
        self.assertEqual(problems(CI, TESTFLIGHT), [])
        self.assertIn('testflight.yml uses a cache',
                      problems(CI, TESTFLIGHT + '      - uses: actions/cache@' + 'a' * 40 + ' # v6.1.0\n'))


if __name__ == '__main__':
    unittest.main()
