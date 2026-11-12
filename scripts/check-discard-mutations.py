#!/usr/bin/env python3
"""SH1-156 mutation negatives. Run only in an exclusively owned Mac checkout.

Each mutant must compile and fail its named behavioral test. A compiler error,
missing toolchain or unrelated failing test is not a passing negative control.
The caller must wrap this process in `just guarded` (one build at a time).
"""
import argparse
from pathlib import Path
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "crates/yebisu/src"
MUTATIONS = [
    ("selection-resume", "terminal_controls.rs",
     "self.handoff.resume_typing();", "/* mutation: retain selection */",
     "sh1_156_selection_new_press_resumes_without_discarded_commit"),
    ("selection-generation", "handoff.rs",
     "self.select_mode = false; // A selection belongs to one destination.",
     "/* mutation: carry Select mode into a new destination */",
     "sh1_156_generation_change_closes_select_mode_before_new_input"),
    ("discard-fence", "terminal_input.rs",
     "// A key press cannot acknowledge native composition cancellation.",
     "self.fresh_gesture();",
     "sh1_156_selection_new_press_resumes_without_discarded_commit"),
    ("repeat-selection", "terminal_controls.rs",
     "if event.is_held { // An old held key cannot end a new selection.",
     "if false {",
     "sh1_156_selection_new_press_resumes_without_discarded_commit"),
]


def run(test):
    command = ["cargo", "test", "--locked", "--workspace", test, "--", "--nocapture", "--test-threads=1"]
    print("COMMAND", repr(command), flush=True)
    result = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    print(result.stdout, end="", flush=True)
    print("EXIT", result.returncode, flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="list without modifying or building")
    args = parser.parse_args()
    if args.list:
        for name, path, _, _, test in MUTATIONS:
            print(name, path, test)
        return 0
    if sys.platform != "darwin":
        sys.exit("Mac build lease required; this script does not build on Linux")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        sys.exit("Refusing to mutate a dirty checkout")
    if run("sh1_156_").returncode:
        sys.exit("Baseline failed: do not interpret mutants")
    for name, relative, old, new, test in MUTATIONS:
        path = SRC / relative
        original = path.read_text()
        if original.count(old) != 1:
            sys.exit(f"{name}: mutation anchor count is not one")
        try:
            path.write_text(original.replace(old, new))
            result = run(test)
            if (result.returncode != 101 or "panicked at" not in result.stdout
                    or f"::{test} ... FAILED" not in result.stdout):
                sys.exit(f"{name}: no named behavioral assertion failure; negative NOT proven")
            print(f"MUTATION PASS {name}: named test failed with an assertion", flush=True)
        finally:
            path.write_text(original)
        if run(test).returncode:
            sys.exit(f"{name}: restored test failed")
    return 0


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    raise SystemExit(main())
