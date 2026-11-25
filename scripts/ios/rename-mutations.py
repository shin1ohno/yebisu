#!/usr/bin/env python3
"""Run the rename-in-place source mutations (issue #143), requiring each named
test to pass on the original source and fail with an assertion on the mutation.

Same shape as layout-mutations.py: each test runs once on the original source
and once on the mutation, through scripts/guarded.py; the source is restored
byte for byte. Durations go to the log and to target/rename-mutations/verdict.json.

  rename-mutations.py [NAME ...]   # all cases, or only the named ones
"""
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/yebisu/src"
OUT = ROOT / "target/rename-mutations"
T = "shell::rename::tests::"

# (name, file, original text, mutated text, test that must fail)
CASES = [
    # B-3: Rename is no longer exempt from "Open this space first".
    ("elsewhere-disables-rename", "shell/host_menus.rs",
     ".partition(|e| e.item == MenuItemId::Space(SpaceMenuItem::Rename));", ".partition(|_| false);",
     T + "entry::rename_enabled_on_another_space_row"),
    # A pick after its space left the snapshot still runs.
    ("rename-scope-ignores-removal", "shell/host_menus.rs",
     "shown.host == current.host && shown.space.is_some() && space_exists",
     "shown.host == current.host && shown.space.is_some()",
     T + "entry::rename_pick_refused_after_space_removed"),
    # A single tap on a space's name renames it (and stops the row's move).
    ("space-single-tap", "shell/rename/entry.rs", "if event.click_count() != 2 {", "if event.click_count() == 0 {",
     T + "entry::double_tap_space_moves_once"),
    # A double tap on a tab's label no longer renames it.
    ("tab-double-tap-ignored", "shell/tab_row.rs", "if event.click_count() == 2 {", "if event.click_count() == 0 {",
     T + "entry::double_tap_tab_moves_once_then_opens"),
    # Rename Space listed in the Terminal menu too.
    ("rename-space-in-terminal-menu", "native_menus.rs",
     "row.info.command != Command::Find && row.info.command != Command::RenameWorkspace",
     "row.info.command != Command::Find", T + "entry::rename_space_only_in_space_menu"),
    # B-7: an empty tab or space name is sent.
    ("empty-tab-sent", "shell/rename/send.rs", "if text.is_empty() && !target.object.allows_empty() {",
     "if false {", T + "send::tab_space_empty_never_sent"),
    # B-7: a key name swapped.
    ("tab-key-swapped", "shell/rename.rs", 'serde_json::json!({ "tab_id": id, "label": label })',
     'serde_json::json!({ "tab": id, "label": label })', T + "send::shapes"),
    # A second commit of one session is sent too.
    ("second-commit-sent", "shell/rename.rs",
     "(!std::mem::replace(&mut s.committed, true)).then(|| s.target.clone())", "Some(s.target.clone())",
     T + "send::one_request_per_commit"),
    # B-7: another request's rejection ends the rename.
    ("reject-any-request", "shell/rename/send.rs", ".filter(|t| Some(t.request.as_str()) == request_id)",
     ".filter(|_| true)", T + "confirm::command_rejected"),
    # B-8 (a): the accepted name is dropped before a newer snapshot.
    ("no-revision-check", "shell/rename/send.rs",
     "let newer = shown.is_some_and(|s| s.revision > pending.sent_revision);", "let newer = true;",
     T + "confirm::confirmed_until_a_newer_snapshot"),
    # B-8 (b): a failure is not reported.
    ("failure-unreported", "shell/rename/send.rs",
     '                self.terminal.live.status = format!("Rename failed: {reason}");\n', "",
     T + "confirm::error_restores_and_reports"),
    # B-8 (c): a disconnect keeps the pending name.
    ("disconnect-keeps-pending", "shell/rename/send.rs",
     "if !live.connected || shown.is_none_or(|s| s.boot_id != pending.boot) {", "if false {",
     T + "confirm::disconnect_clears"),
    # B-8 (d): the timeout waits far longer than 10 s.
    ("no-timeout", "shell/rename/send.rs", "cx.background_executor().timer(CONFIRM_TIMEOUT).await;",
     "cx.background_executor().timer(CONFIRM_TIMEOUT * 60).await;", T + "confirm::timeout_10s"),
    # B-9: the object left the snapshot and the field stays.
    ("removed-target-kept", "shell/rename/slot.rs",
     "Some(s) if s.boot_id != session.target.boot || !object.exists_in(s) => {",
     "Some(s) if s.boot_id != session.target.boot => {", T + "confirm::target_removed_closes"),
    # Q-R5: an anchor missing for two frames keeps the field.
    ("anchor-missing-kept", "shell/rename/slot.rs",
     "            } else {\n                2\n            };",
     "            } else {\n                200\n            };",
     T + "confirm::anchor_missing_twice_cancels"),
    # B-7: the rename takes navigation's one slot.
    ("rename-in-pending-request", "shell/rename/send.rs", "        live.rename = Some(RenameTicket {\n",
     "        live.pending_request = Some(request.clone());\n        live.rename = Some(RenameTicket {\n",
     T + "isolation::pending_request_untouched"),
    # B-6: the composition is not dropped before the field opens.
    ("no-detach", "shell/rename.rs",
     "        self.terminal.input.detach();\n        let revision_at_open",
     "        let revision_at_open", T + "terminal::detach_before_open"),
    # B-6: the handle is dropped at the commit, before Dismissed.
    ("drop-at-commit", "shell/rename.rs", "                if let Some(target) = target {\n",
     "                if let Some(target) = target {\n                    self.with_rename(|s| s.control = None);\n",
     T + "terminal::order_committed_dismissed_restored"),
    # #138's gate widened: the rename field no longer blocks a text retirement.
    ("retirement-ignores-rename", "input_retirement.rs",
     "        if self.native_field_open() {\n            return;\n        }\n        if self.terminal.input.requires_gesture()",
     "        if self.search_open() {\n            return;\n        }\n        if self.terminal.input.requires_gesture()",
     T + "terminal::no_retirement_while_rename_open"),
    # AC15: Herdr 2 treated as Herdr 1.
    ("herdr2-as-herdr1", "shell/rename.rs",
     "        if target.kind == HerdrKind::Herdr2 {\n            return Some(HERDR2);",
     "        if false {\n            return Some(HERDR2);", T + "herdr2::no_request"),
    # AC15: the palette row's reason line is gone.
    ("palette-reason-gone", "action_view.rs", "(rename && !enabled && self.terminal.core_mode)",
     "(false && rename && !enabled && self.terminal.core_mode)", T + "herdr2::palette_row_says_why"),
    # B-7: a name is no longer cut to 160 characters.
    ("no-160-cut", "shell/rename/normalize.rs", "line.trim().chars().take(MAX_CHARS).collect()",
     "line.trim().chars().collect()", T + "normalize::cuts_to_160_characters"),
]


def test_panics(test):
    """`shell::rename::tests::x::y` panics in shell/rename_tests/ (or the module's own tests)."""
    module = test.split("::tests::")[0].replace("::", "/")
    return (f"panicked at crates/yebisu/src/{module}_tests/", f"panicked at crates/yebisu/src/{module}_tests.rs",
            f"panicked at crates/yebisu/src/{module}/tests/")


def run(name, test):
    started = time.monotonic()
    result = subprocess.run(["python3", "scripts/guarded.py", "cargo", "test", "--locked", "-p", "yebisu", test,
                             "--", "--exact", "--nocapture"], cwd=ROOT, capture_output=True, text=True)
    seconds = round(time.monotonic() - started, 1)
    output = result.stdout + result.stderr
    (OUT / (name + ".log")).write_text(output)
    return result.returncode, output, seconds


def main(only):
    OUT.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    results = []
    for name, filename, old, new, test in CASES:
        if only and name not in only:
            continue
        path = SRC / filename
        original = path.read_text()
        if original.count(old) != 1:
            raise SystemExit(f"{name}: source anchor must occur exactly once in {filename}")
        code, output, baseline = run(name + "-baseline", test)
        if code or "1 passed; 0 failed" not in output:
            raise SystemExit(f"{name}: baseline did not run and pass the named regression")
        print(f"rename-mutations: {name}: baseline {test} passed in {baseline}s", flush=True)
        try:
            path.write_text(original.replace(old, new))
            code, output, mutated = run(name, test)
            if code == 0 or f"{test} ... FAILED" not in output or not any(p in output for p in test_panics(test)):
                raise SystemExit(f"{name}: intended assertion failure not observed")
        finally:
            path.write_text(original)
        print(f"rename-mutations: {name}: mutated {test} failed with an assertion in {mutated}s", flush=True)
        results.append({"mutation": name, "test": test, "exit": code, "assertion_failed": True,
                        "baseline_seconds": baseline, "mutation_seconds": mutated})
    total = round(time.monotonic() - started, 1)
    print(f"rename-mutations: {len(results)} mutations killed in {total}s", flush=True)
    (OUT / "verdict.json").write_text(json.dumps({"cases": results, "total_seconds": total}, indent=2) + "\n")


if __name__ == "__main__":
    main(set(sys.argv[1:]))
