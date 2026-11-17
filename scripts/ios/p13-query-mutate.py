#!/usr/bin/env python3
"""Leased native P13 D6/wrong-Cut-caret/Cut-first-query-order assertions, with exact restores."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--expected-head', required=True)
    args = parser.parse_args()
    if platform.system() != 'Darwin' or os.environ.get('HERDR_RETIREMENT_LEASE') != 'exclusive':
        raise RuntimeError('P13 native query mutations require a new exclusive Mac lease')
    head = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    if head != args.expected_head or subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain'], text=True):
        raise RuntimeError('P13 query mutations require a clean exact-head checkout')
    spec = importlib.util.spec_from_file_location('p13_queries', Path(__file__).with_name('p13-query-check.py'))
    queries = importlib.util.module_from_spec(spec); spec.loader.exec_module(queries)
    directory = args.directory.resolve(); directory.mkdir(parents=True, exist_ok=False)
    manifest = {'head': head, 'sdk': queries.SDK, 'cases': []}

    def built(label):
        return json.loads((directory / label / 'provenance.json').read_text())

    def invoke(label, case=None):
        command = [sys.executable, str(Path(__file__).with_name('p13-query-check.py')), str(directory / label), '--run', '--expected-head', head]
        if case: command += ['--mutation-case', case]
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=1800)
        (directory / (label + '.stdout')).write_text(result.stdout)
        (directory / (label + '.stderr')).write_text(result.stderr)
        (directory / (label + '.exit')).write_text(str(result.returncode) + '\n')
        return result

    baseline = invoke('baseline')
    if baseline.returncode or 'receipt validation PASS' not in baseline.stdout:
        raise RuntimeError('P13 native query baseline did not pass')
    base = built('baseline')
    manifest['baseline'] = {'tree': base['tree'], 'executable_sha256': base['installed']['executable_sha256'], 'compiler_fresh': base['compiler']['fresh']}
    for case, (name, _, _, assertion) in queries.CASES.items():
        source = ROOT / name
        original = source.read_bytes()
        candidate = queries.mutate(original.decode(), case).encode()
        failure = None
        mutant = None
        try:
            source.write_bytes(candidate)
            result = invoke(case + '-removed', case)
            if result.returncode != 1 or ('ValueError: ' + assertion) not in result.stderr or 'could not compile' in result.stderr:
                raise RuntimeError(case + ': did not fail the intended actual native receipt assertion')
            # The failing receipt came from THIS mutant's own rebuilt, installed binary.
            mutant = built(case + '-removed')
            if (mutant['changed'] != {name: hashlib.sha256(candidate).hexdigest()} or mutant['compiler']['fresh'] is not False
                    or mutant['installed']['executable_sha256'] == base['installed']['executable_sha256']):
                raise RuntimeError(case + ': mutant run provenance is not the declared rebuilt mutant')
        except BaseException as error:
            failure = error
        finally:
            source.write_bytes(original)
            if source.read_bytes() != original: raise RuntimeError('P13 native source restore differs')
        restored = invoke(case + '-restored')
        if restored.returncode or 'receipt validation PASS' not in restored.stdout:
            raise RuntimeError(case + ': restored native query run failed')
        again = built(case + '-restored')
        if again['tree'] != base['tree'] or again['changed'] or again['status_porcelain']:
            raise RuntimeError(case + ': restored run source differs from baseline')
        manifest['cases'].append({'case': case, 'source': name, 'assertion': assertion,
                                  'original_sha256': hashlib.sha256(original).hexdigest(),
                                  'mutant_sha256': hashlib.sha256(candidate).hexdigest(),
                                  'restored_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                                  'mutant_executable_sha256': mutant and mutant['installed']['executable_sha256'],
                                  'restored_executable_sha256': again['installed']['executable_sha256'],
                                  'restored_matches_baseline_executable': again['installed']['executable_sha256'] == base['installed']['executable_sha256'],
                                  'restored_compiler_fresh': again['compiler']['fresh'],
                                  'intended_negative': failure is None})
        (directory / 'results.json').write_text(json.dumps(manifest, indent=2) + '\n')
        if failure: raise failure
    if subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain'], text=True):
        raise RuntimeError('P13 query mutations left dirty source')
    print('P13 D6, wrong-Cut-caret and Cut-first-query-order native assertion/restoration PASS', flush=True)


if __name__ == '__main__':
    main()
