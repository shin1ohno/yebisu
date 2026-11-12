#!/usr/bin/env python3
"""Leased Mac only: prove D1-D6 regression assertions detect removed fixes.

Run through `just guarded` on an isolated, clean, exact-head checkout. Each
mutation must fail its named runtime assertion, then restored source must pass
the same targeted test. A compile failure never counts as mutation evidence.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[2]
CASES = [
    ("D1", "crates/yebisu/src/find/peer.rs", "let mut serial = surface().surface_revision;", "let mut serial = 0;", "d1_peer_resize_owns_a_monotonic_surface_serial", "D1 resize"),
    ("D2", "crates/yebisu/src/find_view.rs", 'this.actions.consumed.insert("escape".into());', '/* removed UI press ownership */', "d2_close_escape_owns_repeat_and_release_in_both_report_modes", "D2 closing Escape lost press ownership"),
    ("D3", "crates/yebisu/src/find/field.rs", "if let Some(span) = retained_mark {", "if let Some(span) = retained_mark.filter(|_| false) {", "d3_native_partial_mark_delete_replace_then_escape_keeps_search_open", "D3 native partial deletion committed remaining mark"),
    ("D4-nativeCut", "crates/yebisu/src/find/field.rs", 'Some(self.input.selected.clone()),', 'None,', "d4_native_cut_preserves_unselected_mark_then_escape", "D4 Cut deleted unselected marked text"),
    ("D4-commandX", "crates/yebisu/src/find/field.rs", '"x" => self.cut(window, cx),', '"x" => { self.copy(cx); self.replace(None, "", false, None, window, cx); }', "d4_command_x_preserves_unselected_mark_then_escape", "D4 Cut deleted unselected marked text"),
    ("D5", "crates/yebisu/src/terminal_input.rs", 'self.composition = Input::default();\n        self.cancelled = false;\n        true', 'self.composition = Input::default();\n        self.cancelled = true;\n        true', "d5_controlled_close_boundary_recovers_direct_insert_and_mark_without_key_down", "D5 fresh native mark rejected"),
    ("D6", "crates/yebisu/src/find/field.rs", 'Some(self.input.text.encode_utf16().count())', 'Some(self.input.selected.end.max(self.input.marked.as_ref().map_or(0, |r| r.end)))', "d6_document_length_includes_suffix_after_caret_and_partial_emoji_cut", "D6 document end omitted suffix after caret"),
]


def invoke(test, directory, name):
    result = subprocess.run(["just", "guarded", "cargo", "test", "--locked", "--workspace", test, "--", "--nocapture"], cwd=ROOT, text=True, capture_output=True, timeout=1200)
    (directory / (name + ".stdout")).write_text(result.stdout)
    (directory / (name + ".stderr")).write_text(result.stderr)
    (directory / (name + ".exit")).write_text(str(result.returncode) + "\n")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--expected-head", required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise RuntimeError("P13 mutations require an explicit exclusive Mac lease; no Linux product builds")
    head = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
    if head != args.expected_head or subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain"], text=True):
        raise RuntimeError("P13 mutation batch requires a clean exact-head checkout")
    directory = args.directory.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    outcomes = []
    for label, name, before, after, test, assertion in CASES:
        path = ROOT / name
        original = path.read_bytes()
        source = original.decode()
        expected_count = 2 if label == "D2" else 1
        if source.count(before) != expected_count:
            raise RuntimeError(label + " mutation anchor changed")
        try:
            path.write_text(source.replace(before, after))
            result = invoke(test, directory, label + "-removed")
            output = result.stdout + result.stderr
            if result.returncode == 0 or assertion not in output or "could not compile" in output or "error[E" in output:
                raise RuntimeError(label + " did not fail its intended runtime assertion")
        finally:
            path.write_bytes(original)
        restored = invoke(test, directory, label + "-restored")
        if restored.returncode or "1 passed" not in restored.stdout:
            raise RuntimeError(label + " restored runtime regression did not pass")
        outcomes.append({"case": label, "test": test, "assertion": assertion, "restored_sha256": hashlib.sha256(original).hexdigest(), "runtime_negative": "pass", "restored": "pass"})
        (directory / "results.json").write_text(json.dumps({"head": head, "cases": outcomes}, indent=2) + "\n")
    if subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain"], text=True):
        raise RuntimeError("source was not restored after mutation batch")
    print("P13 D1-D6 host runtime mutation assertions and restored tests PASS; UIKit D6 queries require separate native proof", flush=True)


if __name__ == "__main__":
    main()
