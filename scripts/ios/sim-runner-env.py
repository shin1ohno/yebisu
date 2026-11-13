#!/usr/bin/env python3
"""Forward selected raw OS environment entries to a simulator child."""

import os
import sys


def main() -> None:
    if len(sys.argv) < 3 or sys.argv[1] != "--":
        print("usage: sim-runner-env.py -- COMMAND [ARGS...]", file=sys.stderr)
        raise SystemExit(2)

    environment = os.environ.copy()
    for name, value in os.environ.items():
        if name.startswith(("RUST_", "CARGO_", "HERDR_TEST_")):
            environment[f"SIMCTL_CHILD_{name}"] = value

    command = sys.argv[2:]
    os.execvpe(command[0], command, environment)


if __name__ == "__main__":
    main()
