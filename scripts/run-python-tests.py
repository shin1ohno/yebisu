#!/usr/bin/env python3
"""Run the scripts/ unit tests one process per test module, several at a time.

`just test` used to run `python3 -I -B -m unittest discover -s <dir> -p
'test_*.py'` once per directory, serially. This runs the same discovery per
module file in parallel (`discover -s <dir> -p <file>`, the same interpreter
flags) and checks the total against one whole-directory discovery, so a test
that per-file runs would miss fails the run instead of disappearing.

Usage: python3 -I -B scripts/run-python-tests.py [--jobs N] DIR [DIR ...]
Exit 0 only when every module passed and the counts match.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
import fnmatch
import os
from pathlib import Path
import re
import subprocess
import sys
import time

PATTERN = "test_*.py"
RAN = re.compile(r"^Ran (\d+) tests? in ", re.M)
COUNT = (
    "import sys, unittest\n"
    "print(unittest.TestLoader().discover(sys.argv[1], sys.argv[2]).countTestCases())\n"
)


def python(*args):
    return [sys.executable, "-I", "-B", *args]


def modules(directory):
    """The files unittest discover imports from DIR: top-level matches only.

    Discovery also enters packages (directories with __init__.py); none exist
    under scripts/, and the count check would catch tests in one.
    """
    return sorted(
        (path for path in directory.iterdir()
         if path.is_file() and fnmatch.fnmatch(path.name, PATTERN) and path.stem.isidentifier()),
        key=lambda path: (-path.stat().st_size, path.name),  # larger modules first
    )


def run(command):
    start = time.monotonic()
    result = subprocess.run(command, capture_output=True, text=True)
    return result, time.monotonic() - start


def run_module(directory, path):
    result, seconds = run(python("-m", "unittest", "discover", "-s", str(directory), "-p", path.name))
    ran = RAN.findall(result.stderr)
    return {
        "name": f"{directory}/{path.name}", "result": result, "seconds": seconds,
        "tests": int(ran[-1]) if len(ran) == 1 else None,
    }


def count(directory):
    result, _ = run(python("-c", COUNT, str(directory), PATTERN))
    if result.returncode != 0 or not result.stdout.strip().isdigit():
        raise SystemExit(f"run-python-tests: could not count the tests in {directory}:\n{result.stderr}")
    return int(result.stdout)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4, help="parallel modules (default: CPU count)")
    parser.add_argument("dirs", nargs="+", type=Path)
    args = parser.parse_args(argv)
    if args.jobs < 1:
        parser.error("--jobs must be at least 1")

    start = time.monotonic()
    work = [(directory, path) for directory in args.dirs for path in modules(directory)]
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        expected = [pool.submit(count, directory) for directory in args.dirs]
        runs = [pool.submit(run_module, directory, path) for directory, path in work]
        results = [future.result() for future in runs]
        discovered = sum(future.result() for future in expected)

    failed = [r for r in results if r["result"].returncode != 0 or r["tests"] is None]
    for r in sorted(results, key=lambda r: r["name"]):
        mark = "FAIL" if r in failed else "ok  "
        tests = "?" if r["tests"] is None else r["tests"]
        print(f"{mark} {r['name']}: {tests} tests in {r['seconds']:.1f} s")
    for r in failed:
        print(f"\n==== {r['name']} (exit {r['result'].returncode})", file=sys.stderr)
        print(r["result"].stdout + r["result"].stderr, end="", file=sys.stderr)
    ran = sum(r["tests"] or 0 for r in results)
    elapsed = time.monotonic() - start
    print(f"\nRan {ran} tests in {len(results)} modules in {elapsed:.1f} s ({args.jobs} jobs); "
          f"discovery finds {discovered}")
    if ran != discovered:
        print(f"FAILED: ran {ran} tests, but discovery finds {discovered}", file=sys.stderr)
        return 1
    if failed:
        print(f"FAILED: {len(failed)} of {len(results)} modules: "
              + ", ".join(r["name"] for r in failed), file=sys.stderr)
        return 1
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
