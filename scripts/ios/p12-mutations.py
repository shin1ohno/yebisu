#!/usr/bin/env python3
"""Run real Rust source mutations, requiring the named regression to fail."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/yebisu/src"
OUT = ROOT / "target/p12-mutations"
CASES = [
    ("availability", "actions.rs", "                available,", "                available: true,",
     "native_menus::tests::recognition_survives_capability_loss_but_preserves_policy_exclusions"),
    ("duplicate-dispatch", "native_menus.rs",
     ".and_then(|command| self.run_catalog_command(command, Some(action), focused));",
     ".and_then(|command| { self.run_catalog_command(command, Some(action), focused)?; self.run_catalog_command(command, Some(action), focused) });",
     "native_menus::tests::two_native_local_selections_each_execute_once_and_unknown_ids_refuse"),
    ("recognition-filter", "actions.rs", "            a.shortcuts.iter().any(|s| {",
     "            a.available && a.shortcuts.iter().any(|s| {",
     "native_menus::tests::unavailable_shortcut_adapter_consumes_press_repeat_release_without_wire"),
]


def run(name, test):
    result = subprocess.run(["python3", "scripts/guarded.py", "cargo", "test", "--locked", "-p", "yebisu", test, "--", "--nocapture"],
                            cwd=ROOT, capture_output=True, text=True)
    output = result.stdout + result.stderr
    (OUT / (name + ".log")).write_text(output)
    return result.returncode, output


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    for name, filename, old, new, test in CASES:
        path = SRC / filename
        original = path.read_text()
        if original.count(old) != 1:
            raise SystemExit(f"{name}: source anchor must occur exactly once")
        code, output = run(name + "-baseline", test)
        if code or "1 passed; 0 failed" not in output:
            raise SystemExit(f"{name}: baseline did not run and pass the named regression")
        try:
            path.write_text(original.replace(old, new))
            code, output = run(name, test)
            if code == 0 or f"{test} ... FAILED" not in output or "assertion" not in output:
                raise SystemExit(f"{name}: intended assertion failure not observed")
            results.append({"mutation": name, "test": test, "exit": code, "assertion_failed": True})
        finally:
            path.write_text(original)
    (OUT / "verdict.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
