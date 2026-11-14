#!/usr/bin/env python3
"""SH1-156 mutation negatives. Run only in an exclusively owned Mac checkout.

Each mutant must compile and fail its named behavioral test. A compiler error,
missing toolchain or unrelated failing test is not a passing negative control.
The caller must wrap this process in `just guarded` (one build at a time).
"""
import argparse
from pathlib import Path
import re
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
    ("status-reason", "terminal_controls.rs",
     'return "Selection active · press a terminal key or turn Select off to type".into();',
     'return "Composition discarded · tap terminal to type".into();',
     "sh1_156_status_and_highlight_variant_follow_input_gate"),
    ("reconnect-focus", "terminal_view.rs",
     "self.terminal_focus.focus(window, cx);", "/* mutation: leave native focus unrestored */",
     "sh1_156_reconnect_control_restores_focus_and_keys"),
    ("core-destination", "core/worker.rs",
     "== Some(&destination)\n                    {\n                        terminals[saved.active].input(&event)?;",
     "!= None\n                    {\n                        terminals[saved.active].input(&event)?;",
     "sh1_156_core_detach_reattach_uses_new_destination_without_tap"),
    ("paste-fifo", "handoff.rs", "pub fn resume_typing(&mut self) {",
     "pub fn resume_typing(&mut self) { self.cancel();",
     "sh1_156_terminal_key_keeps_existing_paste_fifo"),
    ("navigation-fence", "terminal_controls.rs", ".navigating.is_some()",
     ".navigating.is_some() && false",
     "sh1_156_response_before_projection_keeps_real_key_adapter_fenced"),
    ("repeat-selection", "terminal_controls.rs",
     "if event.is_held && self.handoff.blocks_input() {",
     "self.handoff.resume_typing(); if false {",
     "sh1_156_selection_new_press_resumes_without_discarded_commit"),
    ("clean-detach", "terminal_input.rs", "self.cancelled = cancelled;",
     "self.cancelled = true;", "sh1_156_fresh_herdr1_accepts_printable_and_direct_text"),
    ("lost-discard", "terminal_input.rs", "self.cancelled = cancelled;",
     "self.cancelled = false;", "sh1_156_marked_barrier_retains_discard_fence"),
    ("host-discard", "board.rs", "next.input.retain_discard(discarded);",
     "/* mutation: lose old native client's discard */", "sh1_156_host_switch_inherits_discarded_and_rejected_marks"),
    ("rejected-mark", "terminal_input.rs", "if !text.is_empty() {\n            self.record_discard();\n        }",
     "/* mutation: forget rejected native mark */", "sh1_156_host_switch_inherits_discarded_and_rejected_marks"),
    ("barrier-mark", "terminal_input.rs", "self.reject_mark(text);",
     "/* mutation: forget marked callback during barrier */", "sh1_156_marked_barrier_retains_discard_fence"),
    ("writer-discard", "terminal_controls.rs",
     "self.terminal.input.detach(); // Preserve an actual discard on writer failure.",
     "self.terminal.input = crate::terminal_input::TerminalInput::default();",
     "sh1_156_send_failure_drops_key_but_preserves_real_discard"),
    ("setup-discard", "board.rs", "self.terminal.input.retain_discard(marked);",
     "/* mutation: treat setup composition as fresh terminal text */", "sh1_156_setup_mark_cannot_commit_into_initial_terminal"),
    ("prepare-fence", "input_retirement.rs",
     "self.retirement.prepared = Some(ticket.clone());",
     "self.retirement.prepared = Some(ticket.clone()); self.terminal.input.acknowledge_retirement(ticket.owner.revision);",
     "sh1_156_retirement_prepare_keeps_fence_and_completion_consumes_once"),
    ("completion-prepared", "input_retirement.rs",
     "|| self.retirement.prepared.as_ref() != Some(ticket)",
     "|| false",
     "sh1_156_retirement_unprepared_cancelled_and_changed_completion_remain_fenced"),
    ("completion-owner", "input_retirement.rs",
     "&& ticket.owner == self.retirement_owner()", "&& true",
     "sh1_156_retirement_unprepared_cancelled_and_changed_completion_remain_fenced"),
]


FAILURE_REASONS = {
    "prepare-fence": "prepare opened discard fence",
    "completion-prepared": "invalid completion 0",
    "completion-owner": "invalid completion 3",
    "selection-resume": "fresh key retained Select mode",
    "selection-generation": "reconnect carried selection mode",
    "discard-fence": "key retired no native IME session",
    "status-reason": "SH1-156 selection status reason missing",
    "reconnect-focus": "SH1-156 reconnect focus was not restored",
    "core-destination": "no stale destination, discarded text or replay",
    "paste-fifo": "key cancelled the reserved paste",
    "navigation-fence": "no input to a stale target",
    "repeat-selection": "repeat exited selection",
    "clean-detach": "D1: pristine connection invented a discarded composition",
    "lost-discard": "D1: barrier must preserve real discard without inventing one",
    "host-discard": "D1: host switch lost native discard fence",
    "rejected-mark": "D1: host switch lost native discard fence",
    "barrier-mark": "D1: barrier must preserve real discard without inventing one",
    "writer-discard": "D1: writer failure changed the real discard fence",
    "setup-discard": "D1: setup composition lost its discard fence",
}


def has_build_error(output):
    return any(not line.startswith("error: test failed, to rerun pass ")
               for line in output.splitlines()
               if re.match(r"^error(?:\[E\d+\])?:", line))


def behavioral_failure(name, test, result):
    # Tie the reason to the one named panic, not arbitrary log text from setup
    # or another test. Earlier assertion/compile failures cannot kill a mutant.
    panics = re.findall(r"^thread '([^']+)'[^\n]*panicked at[^\n]*:\n(.*?)(?=^FAILED$|^test |^thread |\Z)",
                        result.stdout, re.MULTILINE | re.DOTALL)
    return bool(result.returncode == 101 and len(panics) == 1
                and panics[0][0].split("::")[-1] == test
                and FAILURE_REASONS[name] in panics[0][1]
                and re.search(r"^\s+(?:\w+::)*" + re.escape(test) + r"$",
                              result.stdout, re.MULTILINE)
                and re.search(r"^test result: FAILED\. 0 passed; 1 failed; 0 ignored;?",
                              result.stdout, re.MULTILINE)
                and not has_build_error(result.stdout))


def behavioral_pass(result, tests):
    # --nocapture can put test output between the name and the final `ok`.
    # cargo exits zero even if a stale filter matches no tests.
    records = re.findall(r"^test ((?:\w+::)*\w+) \.\.\. (.*?)(?=^test |\Z)",
                         result.stdout, re.MULTILINE | re.DOTALL)
    passed = [name.split("::")[-1] for name, body in records
              if body.strip().splitlines()[-1:] == ["ok"]]
    return (result.returncode == 0 and all(passed.count(test) == 1 for test in tests)
            and "panicked at" not in result.stdout and "test result: FAILED" not in result.stdout
            and not has_build_error(result.stdout))


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
    if not behavioral_pass(run("sh1_156_"), {row[4] for row in MUTATIONS}):
        sys.exit("Baseline failed: do not interpret mutants")
    for name, relative, old, new, test in MUTATIONS:
        path = SRC / relative
        original = path.read_text()
        expected_count = 3 if name == "navigation-fence" else 1
        if original.count(old) != expected_count:
            sys.exit(f"{name}: unexpected mutation anchor count")
        try:
            path.write_text(original.replace(old, new))
            result = run(test)
            if not behavioral_failure(name, test, result):
                sys.exit(f"{name}: expected assertion reason missing; negative NOT proven")
            print(f"MUTATION PASS {name}: named test failed: {FAILURE_REASONS[name]}", flush=True)
        finally:
            path.write_text(original)
        if not behavioral_pass(run(test), [test]):
            sys.exit(f"{name}: restored test failed")
    return 0


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    raise SystemExit(main())
