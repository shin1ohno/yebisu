#!/usr/bin/env python3
"""Run the L0 layout geometry source mutations, requiring each named test to
pass on the original source and fail with an assertion on the mutation.

Same shape as p12-mutations.py: each test runs once on the original source and
once on the mutation, through scripts/guarded.py. Durations go to the log and
to target/layout-mutations/verdict.json.
"""
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/yebisu/src"
OUT = ROOT / "target/layout-mutations"
TEST_PANIC = "panicked at crates/yebisu/src/layout/tests/"
CASES = [
    # The terminal's 206 pt minimum comparison (Q1, D2).
    ("minimum-206", "layout/height.rs", "    height >= TERMINAL_MIN_HEIGHT\n", "    height >= 0.0\n",
     "layout::tests::minimum::terminal_never_below_minimum"),
    # pane_mode without the 568 pt comparison: two panes always split (B-2).
    ("pane-mode-568", "layout.rs", "if pane_count >= 2 && column_width >= TWO_PANE_MIN_WIDTH {",
     "if pane_count >= 2 && column_width >= 0.0 {",
     "layout::tests::panes::narrow_column_forces_single_pane"),
    # Height shortage takes M3 before M4 instead of M4 then M3 (q3).
    ("m4-then-m3", "layout/height.rs", "const YIELD_ORDER: [Band; 2] = [Band::Top, Band::Bottom];",
     "const YIELD_ORDER: [Band; 2] = [Band::Bottom, Band::Top];",
     "layout::tests::withdrawal::height_shortage_withdraws_m4_then_shrinks_m3"),
]


def run(name, test):
    started = time.monotonic()
    result = subprocess.run(["python3", "scripts/guarded.py", "cargo", "test", "--locked", "-p", "yebisu", test,
                             "--", "--exact", "--nocapture"], cwd=ROOT, capture_output=True, text=True)
    seconds = round(time.monotonic() - started, 1)
    output = result.stdout + result.stderr
    (OUT / (name + ".log")).write_text(output)
    return result.returncode, output, seconds


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    results = []
    for name, filename, old, new, test in CASES:
        path = SRC / filename
        original = path.read_text()
        if original.count(old) != 1:
            raise SystemExit(f"{name}: source anchor must occur exactly once in {filename}")
        code, output, baseline = run(name + "-baseline", test)
        if code or "1 passed; 0 failed" not in output:
            raise SystemExit(f"{name}: baseline did not run and pass the named regression")
        print(f"layout-mutations: {name}: baseline {test} passed in {baseline}s", flush=True)
        try:
            path.write_text(original.replace(old, new))
            code, output, mutated = run(name, test)
            # The named test must fail on a panic raised in the layout test files
            # (an assert, or a test's own panic), never on a build error or a
            # panic elsewhere. assert!(cond, "msg") prints only the message, so
            # the word "assertion" alone would miss the intended failure.
            if code == 0 or f"{test} ... FAILED" not in output or TEST_PANIC not in output:
                raise SystemExit(f"{name}: intended assertion failure not observed")
        finally:
            path.write_text(original)
        print(f"layout-mutations: {name}: mutated {test} failed with an assertion in {mutated}s", flush=True)
        results.append({"mutation": name, "test": test, "exit": code, "assertion_failed": True,
                        "baseline_seconds": baseline, "mutation_seconds": mutated})
    total = round(time.monotonic() - started, 1)
    print(f"layout-mutations: {len(results)} mutations killed in {total}s", flush=True)
    (OUT / "verdict.json").write_text(json.dumps({"cases": results, "total_seconds": total}, indent=2) + "\n")


if __name__ == "__main__":
    main()
