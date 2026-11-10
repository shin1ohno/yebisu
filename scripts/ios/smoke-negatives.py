#!/usr/bin/env python3
"""Run the real oracle against one independently damaged path at a time."""
import argparse
from pathlib import Path
import subprocess
import sys
import smoke_oracle

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('positive', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
for mode, reason in [('text','text region'), ('quad','quad region'), ('image','image region'), ('input','input result'), ('counter-background','counter did not advance across background'), ('counter-idle','counter did not advance across idle')]:
    destination = args.output / mode
    smoke_oracle.negative_fixture(args.positive, destination, mode)
    result = subprocess.run([sys.executable, str(Path(__file__).with_name('smoke_oracle.py')), str(destination)], capture_output=True, text=True)
    (args.output/(mode+'.txt')).write_text(result.stdout+result.stderr)
    if result.returncode != 1 or reason not in result.stdout:
        raise SystemExit(f'NEGATIVE INVALID {mode}: exit={result.returncode} stdout={result.stdout} stderr={result.stderr}')
    print(f'NEGATIVE PASS {mode}: oracle exit={result.returncode}; {result.stdout.splitlines()[-1]}', flush=True)
