#!/usr/bin/env python3
"""P12 source-contract preflight; NOT Rust, UIKit or physical acceptance."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
FILES = ["native_menus.rs", "native_menus_tests.rs", "native_menus_adapter_tests.rs", "actions.rs", "app.rs", "terminal_controls.rs"]


def violations(sources):
    errors = []
    menu = sources["native_menus.rs"]
    actions = sources["actions.rs"]
    if "&& terminal.navigating.is_none()" in menu:
        errors.append("D1 global navigation gate")
    if "actions::catalog_context_available(terminal)" not in menu:
        errors.append("shared context guard missing")
    if "available == method.as_str()" not in actions:
        errors.append("receiving capability guard missing")
    if "self.consumed = true;" not in menu or "if self.consumed" not in menu:
        errors.append("duplicate dispatch guard missing")
    if "action.revision != Some(self.revision)" not in menu:
        errors.append("stale revision guard missing")
    if "self.scope.as_ref() != Some(&current)" not in menu:
        errors.append("current scope guard missing")
    if "cx.set_menus" not in menu or "Self::native_menu_action" not in sources["app.rs"]:
        errors.append("consumer installation missing")
    if 'Some("P12MenuDisplay")' not in menu:
        errors.append("display binding scope missing")
    if "clear_key_bindings" in menu or 'KeyBinding::new("cmd-c"' in menu or 'KeyBinding::new("cmd-v"' in menu:
        errors.append("competing binding mutation")
    if "shortcut_candidate" not in sources["terminal_controls.rs"]:
        errors.append("availability-only shortcut leakage")
    if "run_menu_shortcut(command)" not in sources["terminal_controls.rs"]:
        errors.append("keyboard adapter bypass")
    return errors


def main():
    sources = {name: (ROOT / "crates/yebisu/src" / name).read_text() for name in FILES}
    assert not violations(sources), violations(sources)
    for file, old, new, reason in [
        ("native_menus.rs", "&& actions::catalog_context_available(terminal)",
         "&& actions::catalog_context_available(terminal) && terminal.navigating.is_none()",
         "D1 global navigation gate"),
        ("actions.rs", "available == method.as_str()", "true", "receiving capability guard missing"),
        ("native_menus.rs", "self.consumed = true;", "self.consumed = false;", "duplicate dispatch guard missing"),
        ("native_menus.rs", "action.revision != Some(self.revision)", "false", "stale revision guard missing"),
    ]:
        assert old in sources[file], "positive control"
        mutated = dict(sources)
        mutated[file] = mutated[file].replace(old, new)
        assert reason in violations(mutated), f"undetected mutation: {reason}"
        print(f"source negative control: {reason}: rejected")
    for file, source in sources.items():
        count = len(source.splitlines())
        assert count <= 1000, (file, count)
        print(f"{file}: {count} lines")
    subprocess.run(["python3", str(ROOT / "scripts/check-sorted.py")], cwd=ROOT, check=True)
    subprocess.run(["git", "diff", "--check"], cwd=ROOT, check=True)
    print("PASS source preflight only; Rust tests/native receiver/screenshot remain unverified")


if __name__ == "__main__":
    main()
