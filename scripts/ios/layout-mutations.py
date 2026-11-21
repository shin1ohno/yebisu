#!/usr/bin/env python3
"""Run the layout source mutations (L0 geometry, L1 settings, L2a frame, L2b sidebar
footer, L2c one-pane rule, L3 sidebar, L4 rail, L5 edit bar, L6 Shortcuts and menus,
L7 Hosts sheet, L8 Herdr 2 frame, L9 keyboards and mod regions), requiring each named test to
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


def test_panics(test):
    """The named test's own test directory. `a::b::tests::c::name` lives in
    a/b/tests/ (L0 layout, L1 settings_pages) or in a/b_tests/ when the module
    keeps its tests beside it (L2b shell/footer_tests/, L2c
    shell/single_pane_tests/ and core/screen_tests/, L3 shell/sidebar_tests/,
    L4 shell/rail_tests/ and shell/rail_badges_tests/, L5
    shell/edit_bar_tests/, L6 shell/shortcuts_tests/ and
    shell/host_menus_tests/, L7 shell/hosts_sheet_tests/, L9 shell/fit_tests/,
    shell/mods_tests/ and shell/disconnect_tests/, L8
    shell/herdr2_frame_tests/), or in a/b_tests.rs (L2a shell/tab_row_tests.rs)."""
    module = test.split("::tests::")[0].replace("::", "/")
    return (f"panicked at crates/yebisu/src/{module}/tests/", f"panicked at crates/yebisu/src/{module}_tests/",
            f"panicked at crates/yebisu/src/{module}_tests.rs")


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
    # L1 (plan v2 7.4): Option key no longer written, so it is lost on restart (Q7).
    ("l1-input-options-saved", "settings/input_options.rs", "    get: |v| v.option_alt,\n",
     "    get: |_| false,\n", "settings_pages::tests::persistence::input_options_survive_restart"),
    # L1: "Saved" shown before the worker confirmed the write (D7).
    ("l1-saved-after-write", "settings.rs", "            None if self.ready() => SaveStatus::Saved,\n",
     "            None => SaveStatus::Saved,\n",
     "settings_pages::tests::save_status::saved_appears_only_after_write_success"),
    # L1: a toolbar change no longer reaches the sheet's draft and store (S-6).
    ("l1-toolbar-sheet-sync", "settings_pages/toolbar_sync.rs", "        let Some(host) = &mut self.settings else {\n",
     "        let Some(host) = None::<&mut crate::settings::SettingsHost> else {\n",
     "settings_pages::tests::toolbar_sync::toolbar_change_is_visible_in_sheet_draft"),
    # L2a (plan v2 7.5): the tab's × never hits, so a tap on it selects (S-9).
    ("l2a-close-hit", "shell/tab_row.rs", "return Some(if x >= close_left {",
     "return Some(if x >= right {",
     "shell::tab_row::tests::close_hit_closes_and_does_not_select"),
    # L2a: Reconnect shows in the status bar while connected too.
    ("l2a-reconnect-condition", "shell/status_bar.rs", "show_reconnect: !state.connected,",
     "show_reconnect: true,",
     "shell::status_bar::tests::reconnect_shows_only_when_disconnected"),
    # L2a: Paste drops out of the entry-point set (the ... menu place, B-1).
    ("l2a-entry-paste", "shell/entry_points.rs", "OverflowItem::Paste => Some(EntryAction::Paste),",
     "OverflowItem::Paste => None,",
     "shell::entry_points::tests::chrome_entry_points_cover_required_set"),
    # L3 (plan v2 7.8): Error ranked after Done breaks the AGENTS order (Q5).
    ("l3-agents-order", "shell/sidebar_status.rs", "            Self::Error => 1,",
     "            Self::Error => 3,",
     "shell::sidebar::tests::order::agents_order_needs_you_error_working_done_idle_unknown"),
    # L3: a status-less terminal counted as an agent (Q4).
    ("l3-status-less", "shell/sidebar_status.rs", "        !matches!(self, Self::NoStatus)\n", "        true\n",
     "shell::sidebar::tests::rows::status_less_terminal_is_not_an_agent"),
    # L3: the notification target resolved by its recorded tab, not by identity (S-2).
    ("l3-notification-target", "shell/sidebar/target.rs", ".find(|p| p.pane_id == agent.pane)",
     ".find(|p| p.pane_id == agent.pane && target.tab.as_ref().is_none_or(|t| t == &p.tab_id))",
     "shell::sidebar::tests::identity::notification_target_resolves_after_identity_change"),
    # L5 (plan v2 7.10): the edit bar drawn without a selection (display condition).
    ("edit-bar-condition", "shell/edit_bar.rs", "    if selecting {\n        vec![", "    if true {\n        vec![",
     "shell::edit_bar::tests::visibility::edit_bar_only_while_selecting"),
    # L5: Cancel no longer lets typing resume.
    ("edit-bar-cancel-resume", "shell/edit_bar.rs", "            self.terminal.input.fresh_gesture();\n",
     "            let _ = ();\n", "shell::edit_bar::tests::cancel::cancel_resumes_typing"),
    # L5: the status bar wording that explains the pause.
    ("edit-bar-wording", "shell/edit_bar.rs",
     'const PAUSED_TYPING_STATUS: &str = "Typing is paused until you cancel the selection";',
     'const PAUSED_TYPING_STATUS: &str = "Typing is paused";',
     "shell::edit_bar::tests::status::status_explains_paused_typing"),
    # L7 (plan v2 7.12): Trust only for the host that was scanned (app_setup.rs:181-186 binding).
    ("l7-trust-binding", "shell/hosts_sheet/actions.rs", "        if target != typed {", "        if false && target != typed {",
     "shell::hosts_sheet::tests::trust::trust_is_bound_to_the_scanned_target"),
    # L7: a Scan or Trust while a host-key job runs (double Scan).
    ("l7-double-scan", "shell/add_host.rs",
     "        !self.scanning && self.step != AddHostStep::SignIn && !self.typed_host.trim().is_empty()",
     "        self.step != AddHostStep::SignIn && !self.typed_host.trim().is_empty()",
     "shell::hosts_sheet::tests::scan::scan_in_flight_blocks_second_scan_and_trust"),
    # L7: the Add host sheet opens by itself with zero hosts.
    ("l7-first-run-auto-open", "shell/hosts_sheet/view.rs", ".auto_open(self.setup.loaded, self.setup.hosts.len());",
     ".auto_open(false, self.setup.hosts.len());",
     "shell::hosts_sheet::tests::first_run::first_run_opens_add_host_sheet_over_empty_frame"),
    # L9 (plan v2 7.15): input queued while the window is too small (Q1, D2).
    ("l9-no-send-too-small", "terminal_input.rs", "        if self.size_hold {\n            return false;\n",
     "        if false {\n            return false;\n",
     "shell::fit::tests::too_small::too_small_window_message_and_no_send"),
    # L9: Mods shown with nothing withdrawn (Q3).
    ("l9-mods-condition", "shell/mods.rs", "    (withdrawn > 0).then(|| format!(\"Mods {withdrawn}\"))",
     "    true.then(|| format!(\"Mods {withdrawn}\"))",
     "shell::mods::tests::item::mods_item_only_when_withdrawn"),
    # L9: the disconnected region held 60 s instead of 30 s (mod point 7).
    ("l9-disconnect-30s", "shell/disconnect.rs", "pub(crate) const DISCONNECT_HOLD: Duration = Duration::from_secs(30);",
     "pub(crate) const DISCONNECT_HOLD: Duration = Duration::from_secs(60);",
     "shell::disconnect::tests::fade::disconnected_region_fades_after_30s"),
    # L2b (plan v2 7.6): Hosts removed from the sidebar footer (5.1 rule 2).
    ("l2b-footer-hosts", "shell/footer.rs", "            item: FooterItem::Hosts,\n",
     "            item: FooterItem::NewSpace,\n",
     "shell::footer::tests::items::sidebar_footer_has_new_space_hosts_settings"),
    # L2b: the sidebar header does not open the New space chooser.
    ("l2b-herdr2-new-space", "shell/footer.rs",
     "            .on_click(cx.listener(|this, _, _, cx| this.open_new_space(cx)))",
     "            .on_click(cx.listener(|_, _, _, _| {}))",
     "shell::footer::tests::new_space::sidebar_header_new_space_opens_chooser_on_both_herdrs"),
    # L2b: Settings removed from the sidebar footer.
    ("l2b-footer-settings", "shell/footer.rs", "            item: FooterItem::Settings,\n",
     "            item: FooterItem::Hosts,\n",
     "shell::footer::tests::items::sidebar_footer_has_new_space_hosts_settings"),
    # L4 (plan v2 7.9). The rail's badge fit comparison: a left badge that is too wide still fits.
    ("l4-badge-fit", "shell/rail_badges.rs",
     "    let left_fits = left_right_edge + MIN_GAP <= dot_left;\n",
     "    let left_fits = left_right_edge + MIN_GAP <= dot_left + 100.0;\n",
     "shell::rail_badges::tests::fit::rail_badges_fall_back_to_combined_when_too_wide"),
    # L4: no Combined fallback, two badges are drawn even when they would touch the dot.
    ("l4-combined-fallback", "shell/rail_badges.rs",
     "    } else {\n        RailBadges::Combined\n    }\n",
     "    } else {\n        RailBadges::Two\n    }\n",
     "shell::rail_badges::tests::fit::rail_badges_fall_back_to_combined_when_too_wide"),
    # L4: the sidebar collapses when two panes no longer fit, not when one does not (Q2: collapse last).
    ("l4-collapse-order", "shell/rail.rs",
     "pub(crate) const FULL_MIN_WINDOW: f32 = SIDEBAR_WIDTH + RULE + TERMINAL_MIN_WIDTH;",
     "pub(crate) const FULL_MIN_WINDOW: f32 = SIDEBAR_WIDTH + RULE + crate::layout::TWO_PANE_MIN_WIDTH;",
     "shell::rail::tests::collapse::sidebar_collapses_last"),
    # L2c (plan v2 6.3): Herdr 1 narrow column, the Zoom request is never sent.
    ("l2c-zoom-send", "shell/single_pane.rs",
     "        if available && self.run_action(Command::Zoom).is_ok() {",
     "        if false && available && self.run_action(Command::Zoom).is_ok() {",
     "shell::single_pane::tests::zoom::herdr1_narrow_column_requests_zoom_once"),
    # L2c: Herdr 2 draws every pane in a narrow column instead of the active one.
    ("l2c-herdr2-active-only", "core/screen.rs",
     "        if single && index != saved.active {",
     "        if false && single && index != saved.active {",
     "core::screen::tests::narrow::herdr2_narrow_column_draws_active_pane_only"),
    # L2c: a stacked division under the 206 pt minimum is no longer refused.
    ("l2c-per-pane-minimum", "core/screen.rs",
     "        each.rows < least_rows",
     "        false",
     "core::screen::tests::minimum::per_pane_size_never_below_minimum"),
    # L6 (plan v2 7.11): the Shortcuts rows are the catalog's; narrowing the catalog drops a row.
    ("l6-catalog-filter", "shell/shortcuts.rs", "    catalog\n        .into_iter()\n",
     "    catalog\n        .into_iter()\n        .skip(1)\n",
     "shell::shortcuts::tests::rows::shortcuts_rows_come_from_catalog"),
    # L6: long press of a space row lists the Space menu's items (Q8).
    ("l6-long-press-match", "shell/host_menus.rs",
     "const SPACE_LONG_PRESS: [SpaceMenuItem; 2] = [SpaceMenuItem::NewTab, SpaceMenuItem::Detach];",
     "const SPACE_LONG_PRESS: [SpaceMenuItem; 1] = [SpaceMenuItem::NewTab];",
     "shell::host_menus::tests::long_press::menu_items_match_long_press"),
    # L8 (plan v2 7.14): a status-less Herdr 2 terminal listed in AGENTS (Q4).
    ("l8-status-less", "shell/herdr2_frame/session.rs", ".filter(|(_, status)| status.is_agent())",
     ".filter(|_| true)", "shell::herdr2_frame::tests::session::herdr2_status_less_not_in_agents"),
    # L8: herdr-core's error folded into Needs you, so Error loses its place after it (Q5).
    ("l8-error-order", "shell/herdr2_frame.rs", "Some(StatusState::Error) => SidebarStatus::Error,",
     "Some(StatusState::Error) => SidebarStatus::NeedsYou,",
     "shell::herdr2_frame::tests::session::herdr2_error_after_needs_you"),
    # L8: New space defaults to Herdr 1 on hosts running both kinds.
    ("l8-new-space", "shell/herdr2_frame/new_space.rs",
     "            .and_then(|host| host.kinds.last())",
     "            .and_then(|host| host.kinds.first())",
     "shell::herdr2_frame::new_space::tests::new_space_asks_for_the_host_and_the_herdr"),
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
            if code == 0 or f"{test} ... FAILED" not in output or not any(p in output for p in test_panics(test)):
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
