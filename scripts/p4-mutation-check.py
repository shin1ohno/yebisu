#!/usr/bin/env python3
"""Break one P4 invariant at a time; require the named behavioral assertion to fail.

Run from a clean/non-concurrently edited checkout. Sources are restored even on
failure. Every compile is guarded. This does not change the committed result.
"""
import argparse
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SRC = Path("crates/yebisu/src")
CASES = [
    (SRC / "upstream/terminal_painter.rs", "byte_index(text, range.start)", "byte_index(text, 0)",
     "mid_clause_rotation_keyboard_resize_share_paint_hit_and_candidate_geometry"),
    (SRC / "terminal_input.rs", "replace(None, text, true, selected)", "replace(None, text, true, None)",
     "emoji_utf16_selection_and_reads_do_not_split_a_scalar"),
    (SRC / "input_geometry.rs", "self.grid.origin,", "Point::default(),",
     "mid_clause_rotation_keyboard_resize_share_paint_hit_and_candidate_geometry"),
    (SRC / "terminal_input.rs", "self.ready = self.destination.is_some()", "if size.is_some() { self.discard(); }\n        self.ready = self.destination.is_some()",
     "keyboard_resize_keeps_composition_and_fifo_only_for_same_target"),
    (SRC / "terminal_input.rs", "            self.discard();", "            // Deliberate negative control: retained composition.",
     "remote_pane_change_discards_composition_and_late_commit"),
    (SRC / "terminal_input.rs", "            self.discard();", "            // Deliberate negative control: retained composition.",
     "popup_open_replace_and_close_each_discard_composition"),
    (SRC / "terminal_input.rs", "focused && !setup && self.destination.is_some()", "focused && self.destination.is_some()",
     "setup_focus_never_accepts_terminal_text"),
    (SRC / "input.rs", "if !self.active_focus().is_focused(window)", "if false",
     "uikit_handler_callbacks_preserve_mark_selection_and_setup_isolation"),
    (SRC / "link.rs", "if crate::terminal_input::destination(&live, self.generation).as_ref() != Some(destination)", "if false",
     "receiving_peer_gets_one_semantic_commit_and_rejects_after_disconnect"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--just", required=True)
    args = parser.parse_args()
    for path, before, after, test in CASES:
        source = ROOT / path
        original = source.read_text()
        if before not in original:
            raise RuntimeError("stale mutation anchor: " + str(path))
        try:
            source.write_text(original.replace(before, after, 1))
            subprocess.run(["df", "-g", "/System/Volumes/Data"], check=True)
            result = subprocess.run([args.just, "guarded", "cargo", "test", "--locked", test], cwd=ROOT, text=True, capture_output=True)
            output = result.stdout + result.stderr
            print("P4 MUTATION", path, test, "exit", result.returncode, flush=True)
            print(output, flush=True)
            if result.returncode != 101 or "test result: FAILED" not in output or "panicked at" not in output:
                raise RuntimeError("mutation must fail its behavioral assertion, not compilation/environment: " + test)
        finally:
            source.write_text(original)
    print("P4 mutation controls PASS: 9 behavioral failures, all sources restored", flush=True)


if __name__ == "__main__":
    main()
