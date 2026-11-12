#!/usr/bin/env python3
"""Leased macOS-only D1 adapter mutation. Never accepts native or physical gates."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/yebisu/src/native_menus.rs"
TEST = "native_menus::adapter_tests::pending_rpc_local_callbacks_once_without_extra_wire"
ANCHOR = "&& actions::catalog_context_available(terminal)"
DIAGNOSTIC = "D1 pending local menu must remain enabled"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("Requires controller-granted exclusive Mac lease; no Linux Rust build")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip():
        raise SystemExit("Requires clean exact-head isolated checkout")
    args.output.mkdir(parents=True, exist_ok=True)
    original = SOURCE.read_bytes()
    text = original.decode()
    assert text.count(ANCHOR) == 1, "mutation anchor must be unique"
    mutated = text.replace(ANCHOR, ANCHOR + "\n                && terminal.navigating.is_none()")
    command = ["just", "guarded", "cargo", "test", "--locked", "-p", "yebisu", TEST, "--", "--exact", "--nocapture"]
    records = []

    def run(label):
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=900)
        output = result.stdout + result.stderr
        (args.output / f"{label}.txt").write_text(output)
        records.append({"label": label, "command": command, "exit": result.returncode})
        return result.returncode, output

    try:
        code, output = run("baseline")
        assert code == 0 and "1 passed" in output, "baseline adapter must pass"
        SOURCE.write_text(mutated)
        code, output = run("d1-mutant")
        assert code != 0 and DIAGNOSTIC in output and "1 failed" in output, \
            "mutant must fail the specific D1 assertion, not a build/environment error"
    finally:
        SOURCE.write_bytes(original)
        (args.output / "mutation-record.json").write_text(json.dumps({
            "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "source_sha256": hashlib.sha256(original).hexdigest(),
            "restored": SOURCE.read_bytes() == original,
            "runs": records,
        }, indent=2) + "\n")
    code, output = run("restored")
    assert code == 0 and "1 passed" in output, "restored adapter must pass"
    (args.output / "mutation-pass.json").write_text(json.dumps({
        "test": TEST, "diagnostic": DIAGNOSTIC, "runs": records,
        "source_restored": SOURCE.read_bytes() == original,
        "native_acceptance": False,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
