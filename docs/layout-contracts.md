# SH1-157 layout contracts (Wave 0)

This is the contract for the overnight layout sprint of 2026-11-18. It fixes,
before any stream starts, which stream owns which file, the names and
signatures each stream fills, which other streams' items it may call, and the
test names it must add. The design is plan v2 (SH1-285,
`2783770380804b80/plan-2.md`, sections 4 to 7); the process is the approved
sprint plan (`mellow-squishing-wilkinson.md`). F1 (herdr-gpui
`sheprd-core-client` `CoreStatus`) and L0 (`crates/yebisu/src/layout`) are on
`main` and are not streams here.

Branch: `layout/integration`, cut from `origin/fix/hw-keyboard-after-discard-3`
(PR 70's head `cad6d74`), with `origin/main` 492c604 merged in. One
integrator owns it. Integration 1 merged `origin/main` aed154e (PR 70
c33b746, PR 71) and the Wave 1 streams listed in section 7.

The amendments of section 8 (review 0e80, 2026-11-18) override the text
above and below where they differ. Section 9 is the contract of a later
plan, the design fidelity and fork port of 2026-11-20.

## 1. Rules for every stream

1. Branch from `layout/integration` as `layout/<stream>` (for example
   `layout/l2a`). Push only that branch with an explicit refspec. Open no PR;
   the integrator merges the branch into `layout/integration` at the wave end.
   No rebase and no force push; take upstream changes with
   `git merge origin/layout/integration`.
2. Edit only the files your stream owns (section 4). A file nobody owns stays
   unchanged unless a patch to the integrator says otherwise (section 3).
3. Fill the Wave 0 stubs in place: keep each name and signature in section 5.
   Adding private helpers, new fields inside your own `*State` struct, and new
   files under your own prefix is free. Changing a listed signature is a
   contract change: ask the integrator first (section 3).
4. Tests go in a separate file next to the module, declared at the end of the
   owner's module file as
   `#[cfg(test)] #[path = "<module>_tests.rs"] mod tests;`
   (for `shell/frame.rs` the file is `shell/frame_tests.rs`). Use the exact
   test names in section 5.
5. Repository rules (AGENTS.md): no file over 1,000 lines, no `unwrap` or
   `expect` in production code, test bodies outside production files,
   `keep-sorted` regions sorted, conventional commit subjects
   (`feat|fix|perf|refactor|revert|docs|chore|ci|build|style|test`) written
   with `git commit -F <file>` and checked with
   `python3 scripts/release/check-commit-messages.py range origin/main..HEAD`
   before each push, no AI attribution.
6. Verify on this host before pushing:
   `cargo test -p yebisu` and
   `cargo clippy -p yebisu --all-targets -- -D warnings`
   (the sprint decision allows local runs tonight). macOS-only checks run in
   the hosted CI of the integration PR at the wave end.
7. Keep plan v2 5.1: a stream that removes an old place adds the new place in
   the same change; `chrome_entry_points_cover_required_set` (L2a) stays green
   on every stream branch from L2a on.

## 2. Waves

| Wave | Streams (start together) | Upstream they build against |
|---|---|---|
| 0 | Integrator: this contract and the skeleton | PR 70 head |
| 1 | T0, L1, L2a, L3 (component), L7 (component); then L5 when T0 is done | Wave 0 stubs |
| 2 | L2b, L2c, L4, L6 (plus the rest of L5) | Wave 1 merged into `layout/integration` |
| 3 | L8, L9 | Wave 2 merged |

Plan v2's dependency graph (4.2) is kept, but a downstream stream does not wait
for its upstream to land. Before the upstream lands, a stream:

- builds against the Wave 0 stubs: calling another stream's function returns
  the stub's inert value (empty list, `None`, `false`, or today's behaviour);
- tests its own rules with its own inputs (models, fake rows, explicit
  widths), never by asserting another stream's output;
- does not copy or edit the upstream's file to get real behaviour;
- after the upstream lands, merges `origin/layout/integration` into its branch
  and reruns its tests.

"Component" for L3 and L7 means: the sidebar model and view (L3) and the Hosts
sheet and Add host flow (L7) are built and tested as components that the
frame calls through the slots below. They replace the board and the setup page
on screen only when L2a's frame (L3) and L2b's footer (L7) are in the
integration branch.

**L0 is in `layout/integration`.** The integrator merged `origin/main`
(492c604: L0, PR 68, and PR 69) with a merge commit, so `crate::layout` is
available to every stream from the Wave 1 base. When PR 70 merges into `main`,
the next integrator merges `origin/main` again; streams take it through
`git merge origin/layout/integration`.

L0's public items used by the streams (from `origin/main` 492c604,
`crates/yebisu/src/layout.rs`): `pub fn compute(input: &Input) -> Layout`,
`pub fn pane_mode(column_width: f32, pane_count: usize) -> PaneMode`,
`pub struct Input { window, insets, regions, sidebar_list_height,
status_reserved_width, find_bar_open, qa_strip_height, pane_count }`,
`pub struct Layout { sidebar, sidebar_collapsed, tab_row, find_bar, status_bar,
m1, m2, m3, m4, m5, m6, overlay, qa_strip, terminal: TerminalFit, withdrawn,
pane_mode }`, `pub enum TerminalFit` with `fn rect(self) -> Option<Rect>`,
`pub struct TooSmall { available, minimum }`, `pub enum Region`,
`pub enum PaneMode { Single, Split }`, and the constants `SIDEBAR_WIDTH`,
`RAIL_WIDTH`, `TAB_ROW_HEIGHT`, `STATUS_BAR_HEIGHT`, `FIND_BAR_HEIGHT`,
`SIDEBAR_HEADER_HEIGHT`, `SIDEBAR_FOOTER_HEIGHT`, `TERMINAL_MIN_WIDTH`,
`TERMINAL_MIN_HEIGHT`, `TWO_PANE_MIN_WIDTH`, `TOAST_SIZE`.

## 3. Asking the integrator for a change

Integrator-only files (only the integrator edits them):

- `crates/yebisu/src/main.rs`
- `crates/yebisu/src/app.rs`
- `crates/yebisu/src/terminal_view.rs`
- `.github/workflows/ci.yml`
- `docs/device-check.md`
- `TODO.md`

Integrator-owned shared files (also changed only by the integrator, from
patches): `crates/yebisu/src/shell.rs`, `crates/yebisu/src/shell/tests.rs`,
`docs/layout-contracts.md`, `crates/yebisu/src/notification_view.rs`,
`crates/yebisu/src/observation.rs`, `crates/yebisu/src/mock_peer.rs`,
`scripts/ios/layout-mutations.py`, `scripts/ios/layout-check.py` (A2),
`justfile`, `Cargo.toml`, `Cargo.lock`,
`crates/yebisu/Cargo.toml`, and every `scripts/ios/smoke-qa/Tests/*.swift`
change after T0 has landed.

To ask for a change to one of them:

1. Make the change in your own clone, then write it as a patch file in your
   task directory:
   `git diff origin/layout/integration -- <files> > <task dir>/integrator-patches/NN-<short-name>.patch`
   (`NN` counts from 01 per stream).
2. Next to it, write `NN-<short-name>.md`: the stream, the reason in one or two
   sentences, and the test that needs it.
3. List the patch file in your envelope's `artifacts` and in your report.
4. The integrator applies patches with `git apply --3way` at the wave end, in
   wave order, and records each one in its own report. A patch that does not
   apply comes back as a question to the stream, not a silent edit.

Do not commit integrator-only files on your branch: the integrator rejects a
branch that changes them.

## 4. File ownership

Each path has exactly one owner. A `*_tests.rs` file belongs to the owner of
its module. "New" means the stream creates it.

| Owner | Files |
|---|---|
| Integrator | the integrator-only and integrator-owned files of section 3 |
| T0 | `scripts/ios/smoke-qa/Tests/SmokeQA.swift`; new `scripts/ios/smoke-qa/Tests/SmokeQA+Keys.swift`, `SmokeQA+Terminal.swift`, `SmokeQA+Input.swift`, `SmokeQA+Handoff.swift`, `SmokeQA+Settings.swift`, `SmokeQA+Core.swift`, `SmokeQA+Board.swift`; new `scripts/ios/smokeqa-labels-check.py`, `scripts/ios/smokeqa-labels-allow.txt`, `scripts/ios/test_smokeqa_labels_check.py`, `scripts/ios/layout-smoke.py` |
| L1 | `crates/yebisu/src/settings_pages/**` (including the Wave 0 `sections.rs`, `toolbar_sync.rs`), `settings.rs`, `settings/**`, `settings_sheet.rs`, `settings_sheet/**`, `settings_view.rs`, `p15_settings_view.rs`, `notification_settings.rs`, `terminal_controls.rs`, `terminal_controls_tests.rs`, `scripts/ios/p10-check.py` |
| L2a | `crates/yebisu/src/shell/frame.rs`, `shell/tab_row.rs`, `shell/overflow_menu.rs`, `shell/status_bar.rs`, `shell/entry_points.rs`, `shell/toast.rs`, `find_view.rs`; new `scripts/ios/layout-check-l2a.py` and `scripts/ios/p7-check.py` (A2) |
| L2b | `shell/entry_move.rs`, `shell/footer.rs`, `shell/qa_strip.rs`, `actions.rs`, `action_tests.rs`, `action_view.rs` |
| L2c | `shell/single_pane.rs`, `core/screen.rs` |
| L3 | `shell/sidebar.rs`, `shell/sidebar/**`, `shell/sidebar_tests/**`, `shell/sidebar_status.rs`, `board.rs`, `board_tests.rs`, `notifications.rs`, `notifications/**` |
| L4 | `shell/rail.rs`, `shell/rail_badges.rs`, `shell/filter.rs` |
| L5 | `shell/edit_bar.rs`, `shell/edit_bar_tests/**` |
| L6 | `shell/shortcuts.rs`, `shell/shortcuts/**`, `shell/shortcuts_tests/**`, `shell/host_menus.rs`, `shell/host_menus_tests/**`, `native_menus.rs`, `native_menus_tests.rs`, `scripts/ios/p12-mutations.py` |
| L7 | `shell/hosts_sheet.rs`, `shell/hosts_sheet/**`, `shell/hosts_sheet_tests/**`, `shell/add_host.rs`, `app_setup.rs`, `setup.rs`, `setup/**`, `hosts.rs`, `hosts/**`, `scripts/ios/p2-check.py` |
| L8 | `shell/herdr2_frame.rs`, `core.rs`, `core/store.rs`, `core_view.rs`, `core_mock.rs`, `core_mock/**`, `scripts/ios/p9-check.py` |
| L9 | `shell/fit.rs`, `shell/mods.rs`, `shell/overlay.rs`, `shell/disconnect.rs`, `terminal_input.rs`, `terminal_input_tests.rs` |

Paths without a directory are under `crates/yebisu/src/`. Files not in the
table (for example `edit_menu*.rs`, `input.rs`, `live.rs`, `handoff*.rs`,
`EditMenuQA.swift`) stay unchanged in this sprint; a stream that finds it must
change one sends a patch (section 3).

Known shared-file pairs of plan v2 4.2, and how this table resolves them:

| Pair | Plan v2 file | Resolution |
|---|---|---|
| L3, L5, L7 | `shell/status_bar.rs` | L2a owns the status bar; L3 text comes from the sidebar model, L5 from `edit_bar::paused_typing_status`, L9 from `fit::status_bar_overflow` and `mods::mods_item` |
| L3, L5, L7 | `main.rs` mod declarations | all modules are declared in Wave 0 (`shell.rs`) |
| L3, L7 | `observation.rs` | integrator-owned; records go in by patch |
| L4, L6 | `actions.rs` | L2b owns it; `ToggleSidebar` calls `Smoke::toggle_sidebar` (L4, `shell/rail.rs`), `Palette` stays local and L6 reaches it through `Smoke::open_shortcuts` |
| L2a, L9 | toast placement | L2a owns `shell/toast.rs`; it reads L9's `overlay::toast_slot` |
| L2b, L3, L7, L8 | SmokeQA lines | T0 owns the files; later edits are patches |

## 5. Streams

Each block lists: owned modules, the Wave 0 items it fills (exact
signatures as committed), items of other streams it calls, the plan v2
section 7 test names it adds, and the integrator-only files that already call
it (so the stream does not need to edit them).

### T0 — test foundation (plan v2 7.1, 5.4, 5.5)

- **Owns:** the SmokeQA split and the label check (section 4).
- **Fills:** `SmokeQA.swift` split into the seven `extension SmokeQA` files of
  plan v2 5.5 (move only, test names unchanged, each file at most 300 lines);
  `smokeqa-labels-check.py` maps every `tapText("...")` string to a non-test
  source string under `crates/yebisu/src` (prefix match) or to an allow-list
  line with its source file; `layout-smoke.py` runs the 12 mini-only SmokeQA
  tests through the existing `p*-check.py` scripts.
- **Calls:** none.
- **Tests:** `test_smokeqa_labels_check.py` (fails on a SmokeQA string with no
  source, fails when an allow-list source disappears).
- **Integrator patch:** the `layout-smoke` job and the check-job step in
  `.github/workflows/ci.yml` (one patch).

### L1 — settings (plan v2 7.4)

- **Owns:** `settings_pages/**` and the settings store (section 4).
- **Fills:**
  - `settings_pages/sections.rs`:
    `pub(crate) enum Section { Display, Theme, Input, Shortcuts, Connection, Notifications, HostsAndKeys, About }`,
    `pub(crate) fn sections() -> Vec<Section>`,
    `pub(crate) fn section_for_page(_page: Page) -> Option<Section>`.
  - `settings_pages/toolbar_sync.rs`:
    `pub(crate) enum ToolbarSetting { Density, ReturnPolicy, InputOptions }`,
    `impl Smoke { pub(crate) fn toolbar_setting_changed(&mut self, _setting: ToolbarSetting, _cx: &mut Context<Self>) }`.
  - Input's six options, Return (Automatic / Tap) and Saved / Retry in the
    settings store and sheet.
- **Already called from integrator files:** `terminal_view.rs` calls
  `toolbar_setting_changed(ToolbarSetting::Density, cx)` and
  `(ToolbarSetting::ReturnPolicy, cx)`; the Input options toggles live in
  `terminal_controls.rs`, which L1 owns. Remove the Wave 0
  `#[allow(dead_code)]` lines in `settings_pages/mod.rs` once used.
- **Calls:** none from other streams.
- **Tests:** `input_options_survive_restart`,
  `option_alt_setting_changes_key_encoding`, `return_policy_survives_restart`,
  `toolbar_change_is_visible_in_sheet_draft`,
  `sheet_save_updates_toolbar_toggle`,
  `old_store_without_input_options_loads_defaults`,
  `unknown_input_value_falls_back_to_default`,
  `saved_appears_only_after_write_success`,
  `failed_write_keeps_draft_and_offers_retry`,
  `sheet_keeps_position_when_it_fits`, `narrow_sheet_stacks_sections`,
  `settings_sections_follow_canvas_order`, `no_sidebar_section`,
  `every_existing_page_has_a_section`.

### L2a — frame (plan v2 7.5)

- **Owns:** `shell/frame.rs`, `tab_row.rs`, `overflow_menu.rs`,
  `status_bar.rs`, `entry_points.rs`, `toast.rs`, `find_view.rs`,
  `scripts/ios/layout-check-l2a.py` (A2: `layout-check.py` is
  integrator-owned).
- **Fills:**
  - `frame.rs`: `pub(crate) struct FrameState {}`;
    `impl Smoke { pub(crate) fn shell_frame(&mut self, body: AnyElement, _window: &mut Window, _cx: &mut Context<Self>) -> AnyElement; pub(crate) fn board_in_band(&self) -> bool }`.
    `shell_frame` composes, in this order: sidebar column (header with the
    toggle; `render_sidebar_list` or, while it returns `None`, the board;
    `render_sidebar_footer`; `render_rail` when collapsed), tab row
    (`render_tab_row`), find bar, the terminal `body`, `render_edit_bar`,
    `render_qa_strip`, `render_status_bar`. Every slot that returns `None`
    draws nothing.
  - `tab_row.rs`: `pub(crate) struct TabRowMetrics { height: f32, close_width: f32, close_height: f32 }`,
    `pub(crate) enum TabHit { Select(usize), Close(usize), NewTab, Split, Overflow }`,
    `pub(crate) fn tab_row_metrics() -> TabRowMetrics`,
    `pub(crate) fn tab_hit(_x: f32, _tab_widths: &[f32], _row_width: f32) -> Option<TabHit>`,
    `impl Smoke { pub(crate) fn render_tab_row(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - `overflow_menu.rs`: `pub(crate) enum OverflowItem { Paste, PasteImage, SelectText, Copy, Find, CommandPalette, Reconnect, CloseTab, Detach, NewTab }`,
    `pub(crate) struct OverflowEntry { item: OverflowItem, enabled: bool, reason: Option<&'static str> }`,
    `pub(crate) fn overflow_menu_items(_state: &ChromeState) -> Vec<OverflowEntry>`.
  - `status_bar.rs`: `pub(crate) struct StatusBarModel { connection: String, kind: HerdrKind, state: String, show_reconnect: bool, show_sidebar_hint: bool, accessibility_label: String }`,
    `pub(crate) fn status_bar_model(_state: &ChromeState, _connection: &str) -> StatusBarModel`,
    `impl Smoke { pub(crate) fn render_status_bar(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - `entry_points.rs`: `pub(crate) fn chrome_entry_points(_state: &ChromeState, _legacy: LegacyBands) -> Vec<EntryPoint>`,
    `pub(crate) fn required_entry_actions(_state: &ChromeState) -> Vec<EntryAction>`.
    The legacy places (toolbar, bands) are listed here while `legacy` shows
    them; other streams' places come from their `*_entry_points` functions.
  - `toast.rs`: `impl Smoke { pub(crate) fn toast_lift(&self, _window: &Window) -> Pixels }`.
- **Already called from integrator files:** `terminal_view.rs` ends
  `render_terminal` with `self.shell_frame(body, window, cx)` and gates the
  board with `board_in_band()`; `notification_view.rs` adds `toast_lift` to
  the toast's bottom offset.
- **Calls:** L2b `footer::footer_entry_points`, `Smoke::render_sidebar_footer`,
  `Smoke::render_qa_strip`, `Smoke::legacy_bands`; L3
  `Smoke::render_sidebar_list`; L4 `Smoke::render_rail`,
  `rail::rail_entry_points`; L5 `Smoke::render_edit_bar`,
  `edit_bar::paused_typing_status`, `edit_bar::edit_bar_entry_points`; L6
  `Smoke::open_shortcuts`, `shortcuts::shortcuts_entry_points`; L9
  `overlay::toast_slot`, `fit::status_bar_overflow`, `mods::mods_item`; L0
  `layout::compute`.
- **Tests:** `tab_row_metrics`, `close_hit_closes_and_does_not_select`,
  `tab_body_hit_selects_and_does_not_close`,
  `tab_actions_stay_pinned_when_tabs_overflow`,
  `pinned_actions_not_pushed_out_by_long_tab_names`,
  `overflow_menu_items_for_herdr1`, `overflow_menu_items_for_herdr2`,
  `herdr2_overflow_paste_image_enabled`, `status_bar_is_single_line`,
  `reconnect_shows_only_when_disconnected`,
  `status_bar_full_name_in_accessibility_label`,
  `find_bar_renders_at_terminal_column_top`,
  `notification_toast_clears_status_bar`,
  `hidden_sidebar_offers_show_in_status_bar`,
  `chrome_entry_points_cover_required_set`,
  `terminal_receives_allocated_rect`.

### L2b — entry moves (plan v2 7.6)

- **Owns:** `shell/entry_move.rs`, `footer.rs`, `qa_strip.rs`, `actions.rs`,
  `action_tests.rs`, `action_view.rs`.
- **Fills:**
  - `entry_move.rs`: `pub(crate) struct LegacyBands { toolbar: bool, input_options: bool, return_policy: bool, handoff: bool, actions: bool, qa_controls: bool }`,
    `impl LegacyBands { pub(crate) const fn current() -> Self }`,
    `impl Smoke { pub(crate) fn legacy_bands(&self) -> LegacyBands }`.
    L2b turns the bands off in the same change that adds their new places.
  - `footer.rs`: `pub(crate) enum FooterItem { NewSpace, Hosts, Settings }`,
    `pub(crate) struct FooterEntry { item: FooterItem, enabled: bool, reason: Option<&'static str> }`,
    `pub(crate) fn footer_items(_kind: HerdrKind) -> Vec<FooterEntry>`,
    `pub(crate) fn footer_entry_points(_state: &ChromeState) -> Vec<EntryPoint>`,
    `impl Smoke { pub(crate) fn render_sidebar_footer(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - `qa_strip.rs`: `pub(crate) fn qa_strip_enabled() -> bool`,
    `impl Smoke { pub(crate) fn render_qa_strip(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> }`
    (it calls the existing `input_qa_controls`, `handoff_qa_controls`,
    `native_edit_qa_controls`, `edit_menu_qa_controls`, `sh1_156_qa_controls`,
    `sh1_152_controls`, `p13_qa_controls` without changing them).
  - `actions.rs`: `Command::Workspace` in `supported` (New space on Herdr 1).
- **Already called from integrator files:** `terminal_view.rs` gates the
  toolbar, Return, actions, Input options, handoff and QA bands with
  `legacy_bands()`.
- **Calls:** L2a `chrome_entry_points` (test), L7 `Smoke::open_hosts` (footer
  Hosts; `false` keeps today's Keys and hosts page), L1 sections (Settings).
- **Tests:** `sidebar_footer_has_new_space_hosts_settings`,
  `footer_hosts_returns_to_keys_and_hosts`,
  `new_space_sends_workspace_on_herdr1`,
  `new_space_herdr2_disabled_with_reason`, `status_has_no_phase_prefix`,
  `qa_strip_absent_without_qa_env`; keeps
  `chrome_entry_points_cover_required_set` green.
- **Integrator patch:** the SmokeQA lines of plan v2 5.4 marked L2b, and the
  `TITLE`/"P9 ·" text in `app.rs` and `terminal_view.rs`.

### L2c — single pane (plan v2 6, 7.7)

- **Owns:** `shell/single_pane.rs`, `core/screen.rs`.
- **Fills:** `pub(crate) struct SinglePaneState {}`,
  `pub(crate) enum ZoomRequest { Zoom, Release }`,
  `pub(crate) fn herdr1_zoom_request(_state: &SinglePaneState, _host: &str, _tab: u64, _column_width: f32, _pane_count: usize, _zoomed: bool) -> Option<ZoomRequest>`,
  `impl Smoke { pub(crate) fn enforce_pane_mode(&mut self, _window: &mut Window, _cx: &mut Context<Self>) }`;
  Herdr 2 draws only `saved.active` in `core/screen.rs`.
- **Already called from integrator files:** `render_terminal` calls
  `enforce_pane_mode` once per frame.
- **Calls:** L0 `layout::pane_mode`, `layout::TWO_PANE_MIN_WIDTH`; L2a frame
  for the column width.
- **Tests:** `herdr1_narrow_column_requests_zoom_once`,
  `herdr1_forced_zoom_released_on_widen`, `herdr1_user_zoom_not_released`,
  `split_while_narrow_keeps_single_pane`,
  `herdr2_narrow_column_draws_active_pane_only`,
  `per_pane_size_never_below_minimum`.
- **Integrator patch:** a two-pane mock tab in `mock_peer.rs` if missing.

### L3 — sidebar (plan v2 7.8), Wave 1 as a component

- **Owns:** `shell/sidebar.rs`, `sidebar_status.rs`, `board.rs`,
  `board_tests.rs`, `notifications.rs`, `notifications/**`.
- **Fills:**
  - `sidebar_status.rs`: `pub(crate) enum SidebarStatus { NeedsYou, Error, Working, Done, Idle, Unknown, NoStatus }`,
    `pub(crate) fn from_agent_status(_status: AgentStatus) -> SidebarStatus`
    (`sheprd_client::protocol::AgentStatus`; five values, Error and
    NoStatus come from L8).
  - `sidebar.rs`: `pub(crate) struct SidebarState {}`,
    `pub(crate) struct AgentIdentity { host: String, kind: HerdrKind, pane: String }`,
    `pub(crate) struct AgentRow { identity: AgentIdentity, name: String, tab: u64, status: SidebarStatus, received: u64 }`,
    `pub(crate) fn order_agents(_rows: &mut [AgentRow])`,
    `pub(crate) fn display_name(row: &AgentRow, _all: &[AgentRow]) -> String`,
    `pub(crate) fn accessibility_name(row: &AgentRow) -> String`,
    `pub(crate) fn row_height(base: f32, _text_scale: f32) -> f32`,
    `impl Smoke { pub(crate) fn render_sidebar_list(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - Notification targets move to `AgentIdentity` (S-2), which already carries
    the Herdr kind that L8 needs.
- **Calls:** PR 40's `input_transition` for row taps; L2a's frame shows the
  list (until then `render_sidebar_list` is exercised only by its tests).
- **Tests:** `agents_order_needs_you_error_working_done_idle_unknown`,
  `sidebar_status_has_seven_values`, `agent_status_maps_to_sidebar_status`,
  `same_status_newest_received_first`, `cross_daemon_seq_is_ignored`,
  `initial_batch_orders_by_host_then_name`, `identity_survives_rename`,
  `duplicate_names_show_tab_and_pane`,
  `accessibility_name_always_includes_tab_and_pane`,
  `offline_host_shows_no_spaces`, `branch_only_from_herdr1`,
  `status_less_terminal_is_not_an_agent`, `space_row_shows_single_error_tag`,
  `notification_target_resolves_after_identity_change`,
  `row_tap_goes_through_input_transition`, `board_density_toggle_removed`,
  `row_height_follows_text_scale`.
- **Integrator patch:** SmokeQA 434 to 446; the `YEBISU_BOARD_QA` mock in
  `app.rs` if it needs Needs you and Error in one space.

### L4 — rail and filter (plan v2 7.9)

- **Owns:** `shell/rail.rs`, `rail_badges.rs`, `filter.rs`.
- **Fills:**
  - `rail.rs`: `pub(crate) struct RailState {}`,
    `pub(crate) enum SidebarMode { Full, Rail }`,
    `pub(crate) fn sidebar_mode(_window_width: f32, user: SidebarMode) -> SidebarMode`,
    `pub(crate) fn rail_entry_points(_state: &ChromeState) -> Vec<EntryPoint>`,
    `impl Smoke { pub(crate) fn toggle_sidebar(&mut self); pub(crate) fn render_rail(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
    Wave 0 `toggle_sidebar` keeps today's behaviour (`board_hidden ^= true`).
  - `rail_badges.rs`: `pub(crate) enum RailBadges { Two, Combined }`,
    `pub(crate) fn rail_badges(_needs: usize, _error: usize, _measure: &dyn Fn(&str) -> f32) -> RailBadges`.
  - `filter.rs`: `pub(crate) fn filter_rows<'a>(rows: &'a [AgentRow], _query: &str) -> Vec<&'a AgentRow>`.
- **Already called from integrator files:** `actions.rs` (L2b) routes
  `Command::ToggleSidebar` to `toggle_sidebar`.
- **Calls:** L3 `AgentRow`, `SidebarStatus`, `row_height`; L0 `RAIL_WIDTH`,
  `layout::compute` (collapse when 280 does not fit).
- **Tests:** `rail_text_is_at_least_11pt`,
  `rail_full_name_in_accessibility_label`, `rail_two_badges_fit_64`,
  `rail_badges_fall_back_to_combined_when_too_wide`,
  `combined_badge_accessibility_name_keeps_both_counts`,
  `rail_shows_error_badge_only_with_errors`, `sidebar_collapses_last`,
  `header_and_footer_stay_fixed`, `filter_empty_offers_clear`,
  `toggle_sidebar_switches_rail`, `toggle_sidebar_stays_local_command`,
  `row_height_follows_text_scale` (the rail version, in
  `shell/rail_tests.rs`; L3's test of the same name is in
  `shell/sidebar_tests.rs`).

### L5 — edit bar (plan v2 7.10), after T0 in Wave 1

- **Owns:** `shell/edit_bar.rs`.
- **Fills:** `pub(crate) struct EditBarState {}`,
  `pub(crate) enum EditBarItem { Copy, Paste, PasteImage, Cancel }`,
  `pub(crate) fn edit_bar_items(_selecting: bool) -> Vec<EditBarItem>`,
  `pub(crate) fn paused_typing_status(_selecting: bool) -> Option<&'static str>`,
  `pub(crate) fn edit_bar_entry_points(_state: &ChromeState) -> Vec<EntryPoint>`,
  `impl Smoke { pub(crate) fn render_edit_bar(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement>; pub(crate) fn cancel_selection(&mut self, _window: &mut Window, _cx: &mut Context<Self>) }`.
- **Calls:** the existing handoff selection and `input.rs:153` stop (call
  only); L2a frame shows the bar and the status text.
- **Tests:** `edit_bar_only_while_selecting`, `cancel_resumes_typing`,
  `chrome_stays_usable_while_selecting`, `status_explains_paused_typing`.
- **Constraint:** `EditMenuQA.swift` and the edit-menu job stay unchanged.

### L6 — Shortcuts and menus (plan v2 7.11)

- **Owns:** `shell/shortcuts.rs`, `host_menus.rs`, `native_menus.rs`,
  `native_menus_tests.rs`, `scripts/ios/p12-mutations.py`.
- **Fills:**
  - `shortcuts.rs`: `pub(crate) struct ShortcutsState {}`,
    `pub(crate) struct ShortcutRow { label: String, keys: String, enabled: bool }`,
    `pub(crate) fn shortcut_rows(_state: &ChromeState) -> Vec<ShortcutRow>`,
    `pub(crate) fn popover_columns(_width: f32) -> u8`,
    `pub(crate) fn shortcuts_entry_points(_state: &ChromeState) -> Vec<EntryPoint>`,
    `impl Smoke { pub(crate) fn open_shortcuts(&mut self, _cx: &mut Context<Self>) -> bool; pub(crate) fn render_shortcuts_popover(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - `host_menus.rs`: `pub(crate) enum HostMenuItem { Reconnect, Detach, Edit }`,
    `pub(crate) enum SpaceMenuItem { NewTab, Detach }`,
    `pub(crate) struct MenuScope { host: String, space: Option<String> }`,
    `pub(crate) fn scope_still_valid(_shown: &MenuScope, _current: &MenuScope) -> bool`.
- **Already called from integrator files:** `app.rs` layers
  `render_shortcuts_popover` over the screen.
- **Calls:** `actions::catalog` (L2b's file, read only); L3 rows for long
  press; `Command::Palette` stays a local command.
- **Tests:** `shortcuts_rows_come_from_catalog`,
  `narrow_popover_single_column`, `unavailable_rows_are_disabled`,
  `long_press_items_also_in_menu_bar`, `menu_items_match_long_press`,
  `host_menu_item_refused_after_host_switch`,
  `space_menu_item_refused_after_space_change`,
  `menu_edit_host_targets_selected_host`, `palette_stays_local_command`.

### L7 — Hosts sheet (plan v2 7.12), Wave 1 as a component

- **Owns:** `shell/hosts_sheet.rs`, `add_host.rs`, `app_setup.rs`,
  `setup.rs`, `setup/**`, `hosts.rs`, `hosts/**`, `scripts/ios/p2-check.py`.
- **Fills:**
  - `hosts_sheet.rs`: `pub(crate) struct HostsSheetState {}`,
    `impl Smoke { pub(crate) fn open_hosts(&mut self, _cx: &mut Context<Self>) -> bool; pub(crate) fn render_hosts_sheet(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement>; pub(crate) fn render_first_run(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - `add_host.rs`: `pub(crate) enum AddHostStep { Address, Trust, SignIn }`,
    `pub(crate) struct AddHostFlow { step: AddHostStep, typed_host: String, scanning: bool, scanned_fingerprint: Option<String> }`,
    `impl AddHostFlow { pub(crate) fn can_scan(&self) -> bool; pub(crate) fn can_trust(&self) -> bool }`.
- **Already called from integrator files:** `app.rs` layers
  `render_hosts_sheet` over the screen and asks `render_first_run` before
  today's setup page.
- **Calls:** L2b's footer calls `open_hosts`; the existing setup and keys
  code (owned).
- **Tests:** `hosts_footer_opens_host_list`,
  `first_run_opens_add_host_sheet_over_empty_frame`,
  `scan_in_flight_blocks_second_scan_and_trust`,
  `trust_requires_successful_scan`, `mismatch_shows_both_fingerprints`,
  `failed_scan_keeps_typed_host`, `step3_prefers_herdr1`,
  `developer_buttons_only_in_simulator`.
- **Integrator patch:** SmokeQA 46, 49, 372, 375; removing the
  `render_setup` branch of `app.rs` once the first-run screen covers it.

### L8 — Herdr 2 in the same frame (plan v2 7.14), Wave 3

- **Owns:** `shell/herdr2_frame.rs`, `core.rs`, `core/store.rs`,
  `core_view.rs`, `core_mock.rs`, `core_mock/**`, `scripts/ios/p9-check.py`.
- **Fills:** `pub(crate) struct Herdr2State {}`,
  `pub(crate) struct ConnectionKey { host: String, kind: HerdrKind }`,
  `impl Smoke { pub(crate) fn core_controls_in_band(&self) -> bool }`; new
  `pub(crate) fn core_status_to_sidebar(status: Option<&CoreStatus>) -> SidebarStatus`
  in `herdr2_frame.rs`, where `CoreStatus` is F1's type at the pinned
  herdr-gpui rev (the rev bump in `Cargo.toml`/`Cargo.lock` is an integrator
  patch).
- **Already called from integrator files:** `terminal_view.rs` gates the
  Herdr 2 control band with `core_controls_in_band()`.
- **Calls:** L3 `SidebarStatus`, `AgentIdentity`, `order_agents`; L2a
  `overflow_menu_items`; L2b `footer_items`; F1 `CoreStatus`.
- **Tests:** `herdr2_uses_shared_chrome`, `herdr2_status_less_not_in_agents`,
  `herdr2_error_after_needs_you`, `core_status_maps_to_sidebar_status`,
  `herdr2_single_session_is_one_space_one_tab`,
  `new_space_offers_only_herdr1_until_sessions_task`,
  `herdr2_new_tab_disabled_with_reason`, `herdr2_paste_image_enabled`,
  `mixed_host_lists_herdr1_first`, `connection_key_is_host_and_kind`,
  `notification_context_keeps_herdr_kind`,
  `herdr2_rows_accessibility_name_includes_kind`.
- **Integrator patch:** SmokeQA 245 to 257.

### L9 — keyboards and mod regions (plan v2 7.15), Wave 3

- **Owns:** `shell/fit.rs`, `mods.rs`, `overlay.rs`, `disconnect.rs`,
  `terminal_input.rs`, `terminal_input_tests.rs`.
- **Fills:**
  - `fit.rs`: `pub(crate) struct FitState {}`,
    `pub(crate) enum StatusItem { HardwareKeyboard, ModSections }`,
    `pub(crate) fn status_bar_overflow(_available_width: f32) -> Vec<StatusItem>`,
    `impl Smoke { pub(crate) fn render_too_small(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement>; pub(crate) fn terminal_send_allowed(&self) -> bool }`.
  - `mods.rs`: `pub(crate) fn mods_item(_withdrawn: usize) -> Option<String>`,
    `impl Smoke { pub(crate) fn render_mods_overlay(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement> }`.
  - `overlay.rs`: `pub(crate) fn toast_slot() -> Option<(f32, f32)>`.
  - `disconnect.rs`: `pub(crate) enum DisconnectFade { Visible, Dimmed, Cleared }`,
    `pub(crate) fn disconnect_fade(_elapsed: Duration) -> DisconnectFade`.
- **Already called from integrator files:** `render_terminal` returns
  `render_too_small` when it is `Some`; `app.rs` layers
  `render_mods_overlay`.
- **Calls:** L0 `layout::compute`, `TerminalFit`, `TooSmall`; L2a status
  bar; L5 edit bar position (floating keyboard).
- **Tests:** `too_small_window_message_and_no_send`,
  `status_bar_overflow_order`, `floating_keyboard_does_not_resize_terminal`,
  `mods_item_only_when_withdrawn`, `disconnected_region_fades_after_30s`,
  `notification_toast_uses_overlay_slot`, `toast_never_covers_status_bar`.

## 6. Shared types (frozen in Wave 0, `crates/yebisu/src/shell.rs`)

- `pub(crate) enum HerdrKind { Herdr1, Herdr2 }` (default `Herdr1`).
- `pub(crate) struct ChromeState { kind: HerdrKind, connected: bool, selecting: bool, sheet_open: bool, find_available: bool, sidebar_hidden: bool }`.
- `pub(crate) enum EntryAction { Hosts, Settings, NewSpace, Paste, PasteImage, Select, Copy, Reconnect, Find, NewTab, Split, Shortcuts, ToggleSidebar }`.
- `pub(crate) enum Place { Toolbar, InputBand, ReturnBand, HandoffBand, ActionsBand, CoreBand, SidebarFooter, SidebarHeader, TabRow, OverflowMenu, StatusBar, Keyboard, LongPress, EditBar, ShortcutsPopover, MenuBar, Rail, HostsSheet, QaStrip }`.
- `pub(crate) struct EntryPoint { action: EntryAction, place: Place, enabled: bool, reason: Option<&'static str> }`.
- `pub(crate) struct ShellState { frame, sidebar, rail, edit_bar, shortcuts, hosts_sheet, single_pane, herdr2, fit }`,
  held as `Smoke::shell`. Each field is the owning stream's `*State`.

A stream that needs a new shared type or variant sends a patch (section 3).
The `#[allow(dead_code)]` on `mod shell;` in `main.rs` and on `Smoke::shell`
goes away at the end of Wave 3.

## 7. Not done in Wave 0 (TODO for later integrators)

- Done in Integration 1: `origin/main` aed154e merged (PR 70 c33b746 and
  PR 71); `layout/t0` 24a8c3a, `layout/l1` 7ffde18, `layout/l7` 52983b8 and
  `layout/l5` 87350e5 merged with their integrator patches; the
  `layout-smoke` job (T0) with `shell/` and `layout/` in its filter and in
  edit-menu's; the L1, L5 and L7 `layout-mutations` entries; the draft
  integration PR to `main`.
- Integration 1, second push: `layout/l3` 2b859fe with its patches 01 to
  04, and the iOS-only clippy fixes the first hosted run found (L1 test-only
  items, L7's unused `Job::Save`, `trust.rs` gated off iOS). The `*_tests/`
  and sub-module directories the streams created are listed in the table
  above (L3 review N-6).
- Still open: `layout/l2a` and the next `layout/l1` head; Wave 2 and Wave 3
  streams.

## 8. Amendments (2026-11-18, review 0e80 B1 to B4)

- **A1 (B1).** `crates/yebisu/src/shell/tests.rs`
  `wave0_slots_are_inert`: a stream may delete, on its own branch, only the
  asserts for the slots it fills (L2a: `chrome_entry_points`,
  `overflow_menu_items`; L5: `edit_bar_items`, `paused_typing_status`;
  L3 and L2b: `footer_items` and the sidebar slots; and so on), and also
  sends the same lines as a patch. No other change to integrator-only files
  on a branch.
- **A2 (B2).** `scripts/ios/layout-check.py` is integrator-owned. Each stream
  writes its Simulator layout assertions in its own
  `scripts/ios/layout-check-<stream>.py` (for example `layout-check-l1.py`)
  with a `main()` the integrator calls. `p6-check.py` (L3), `p7-check.py`
  (L2a) and `p8-check.py` (L5) are owned by those streams if the stream adds
  them. Host tests are required; Simulator check scripts are best effort and
  may be listed as not done.
- **A3 (B3).** T0's targets follow the branch as it is (`SmokeQA.swift` 790
  lines, 41 `tapText` strings, the existing p2, p3, p9, p10, p8-native,
  sh1-153, sh1-156 and selection-settings checks), not plan v2's 889, 55
  and 12 figures. `layout/t0` already does this.
- **A4 (B4).** Until L2b lands, `required_entry_actions` excludes New space;
  `chrome_entry_points_cover_required_set` in L2a and in the L3, L7 and L5
  branches checks the set without New space. L2b adds New space to the
  required set in the same branch that adds the footer entry.
- **L2a.** The `sh1_150` test is widened from 1200 to 1500 pt (the legacy band
  wraps and the frame jumps); L2b restores 1200 and asserts no jump.
- **A5.** L7 F1 (keys reach the remote shell while the Hosts sheet is open)
  is a merge condition of L2b (S5).
- **A6 (Integration 1 Sonnet review S2).** Before L2b merges, every input
  guard that checks `settings_modal` also checks the Hosts sheet
  (`input_modal()`), opening the sheet over a terminal does the clean-up of
  `begin_settings_modal` (`hand_keys_to_sheet`), and closing it returns the
  terminal focus and the keyboard (`return_keys_to_terminal`). Landed in
  Integration 2 with L2b patch 02. Every path that opens the sheet over a
  live terminal goes through `hand_keys_to_sheet`: footer Hosts, the L4
  rail's Hosts and L6's Host > Edit. L6's `HOSTS_SHEET_FENCED` is true.
- **Integration 2 (L4 review F1).** `ChromeState.sidebar_forced` is true
  while the width forces the rail; every sidebar toggle (status bar Show
  sidebar, rail, keyboard, Shortcuts popover, `Command::ToggleSidebar`) is
  hidden or disabled with "Window too narrow".
- **Integration 2 (Herdr 2 footer).** With the toolbar retired, Herdr 2
  draws the sidebar column with an empty list and the L2b footer until L8
  fills the list; Integration 1 S3's empty-column rule no longer holds.
- **Integration 2 (layout-mutations).** Streams' patches append to `CASES`
  only; the integrator folds their docstring edits into one change.
- **Integration 3 (L8).** L8 also owns `shell/herdr2_frame/**` and
  `shell/herdr2_frame_tests*`. The Herdr 2 control band retires once every
  band control (Split (2 panes), Pane 1, Pane 2, Toggle split direction,
  Detach) is an enabled entry of the `...` menu (`band_retired_in`, patch
  07); SmokeQA and `smokeqa-labels-check.py` find those texts in the menu.
  Herdr 2 pane sizes are L8's `pane_rects`: one line cell between two panes,
  every pane the same size, and the size sent equals the rect drawn (patch
  06; L2c's `size / count` expectations moved by one cell). Patches 02
  (status poll thread) and 03 (Herdr 2 sidebar session) are held by review
  e85e B1, so Herdr 2 keeps the empty list and the footer of Integration 2.
- **Integration 4 (H and W follow-ups).** Stream H (`layout/h2`) rebuilt
  patches 02 and 03 on main: the Herdr 2 status poll runs on its own
  `yebisu-core-status` thread with a bounded join (`core/status.rs`), test
  builds start without a poll, and `render_sidebar_column` draws the Herdr 2
  host, session and AGENTS rows only once the session has a list. The column
  and its footer stay without a list (main d3755cf). Stream W owns
  `frame_layout` and the sidebar header filter slot; H owns the Herdr 2 arm of
  `render_sidebar_column`. In PR 77 the filter field takes no keyboard focus
  (owner decision 2026-11-19, after review rounds G1, H1, SF1/S1 and F1): it
  reads `Filter · not available yet`, and terminal input is what it was before
  W; TODO.md tracks rebuilding it as a UIKit text-input target with IME
  support.

## 9. Design fidelity plan, Wave 1 (2026-11-20)

The plan makes yebisu follow the canvas "yebisu iPad layout" and ports the
herdr-gpui fork's iPad work (`mellow-squishing-wilkinson.md`, approved
2026-11-20; owner decisions in
`codex-controller-2026-11-12/yebisu-design-fidelity-and-fork-port-decision.json`;
gap IDs such as MAIN-20 from `yebisu-design-gap/gap.md`, fork rows such as
R27 from `yebisu-fork-port/inventory.md`). Wave 0a (this section's PR)
froze the shared values and types below. Sections 1 to 8 describe the
SH1-157 sprint; of them, only the repository rules of section 1 rule 5
apply here.

### 9.1 Rules

1. Each stream branches from `origin/main` after Wave 0a merges (A and D
   after the window-control overlap fix merges), opens one PR to `main`, and
   edits only the files it owns (9.2). A stream that must edit another
   stream's file stops and asks the controller, who serializes the two or
   merges them into one stream.
2. Colours, sizes, weights, radii, shadows and repeated lengths come from
   `shell/style.rs`. A view does not declare its own copy of a canvas value;
   a missing value is a patch to the controller, not a local constant.
3. Unsized text is 13 pt (the root sets `style::text::BASE`); a view sets a
   size only where the canvas differs from 13.
4. The UI typeface stays the iPadOS system font; only sizes and weights
   follow the canvas (owner decision).
5. Each PR puts a Simulator screenshot next to the matching artboard PNG
   (1376×1032 landscape unless the artboard is another size).

### 9.2 File ownership

Paths are under `crates/yebisu/src/`. A `*_tests.rs` file or `*_tests/`
directory belongs to the owner of its module.

| Stream | Files (no other stream edits them) | Work |
|---|---|---|
| A sidebar | `shell/sidebar.rs`, `shell/sidebar/**`, `shell/sidebar_status.rs`, `shell/sidebar_list.rs`, `shell/footer.rs`, `shell/footer/**`, `shell/filter.rs`, the header part of `shell/frame.rs` | MAIN-01 (collapse, `yebisu`, search, ＋), MAIN-02 host collapse, MAIN-04, MAIN-06 to 15, MIX-01 and H2-07 (one list for both Herdrs), MIX-02, agent pins, task and pane names on AGENTS rows (R26), density Dense / Roomy |
| B rail | `shell/rail.rs`, `shell/rail/**`, `shell/rail_badges.rs` | RAIL-02 to 07, the long-press menu (R20) |
| C tab row, menus, status bar | `shell/tab_row.rs`, `shell/overflow_menu.rs`, `shell/status_bar.rs`, `shell/edit_bar.rs`, `shell/shortcuts.rs`, `shell/shortcuts/**`, `shell/mods.rs`, `native_menus.rs` (the file that registers the iPadOS menu bar's items, `Smoke::install_native_menus`) | MAIN-16 to 18, 21, 22, RAIL-01 and R21 (space name ▾), RAIL-08, SEL-01, SEL-03, H2-03 (Herdr 2 Paste image enabled: remove the edit bar's `HERDR2_IMAGE_REASON` gate), H2-04, SC-01 to 04, MM-02, the Copy as one line item's placement (9.5), the Shortcuts hint only with a hardware keyboard (R32), the flaky `overflow_menu_closes_on_an_outside_press_that_stays_out_of_the_terminal` (`shell/frame_overlay_tests.rs`) |
| D terminal surface | `terminal_view.rs`, `find_view.rs`, `upstream/terminal_painter.rs`, `upstream/terminal_painter/**`, selection and copy handling | MAIN-03 (no Find band), SEL-02, SEL-04, MAIN-19 and H2-05 (pane borders; for Herdr 1 first establish that pane geometry is available, else stop and return to the owner), MAIN-23, Copy as one line (R58) |
| E sheets | `shell/hosts_sheet.rs`, `shell/hosts_sheet/**`, `settings_sheet.rs`, `settings_sheet/**`, `settings_pages/**` | ADD-01 to 06 (read the device key when the sheet opens, R15), SET-01 to 08 (Settings › Sidebar returns), close on a backdrop tap (R46) |
| F Herdr 2 | `shell/herdr2_frame.rs`, `shell/herdr2_frame/**` | H2-01 several sessions, H2-02, H2-06, SPEC-01 (New space picks the host and Herdr 1 or 2), SC-02's Herdr 2 commands |
| H1 app behaviour | app start and stop handling | keep the screen awake while connected (R54) |

Wave 0a's own files stay with the controller after it merges:
`shell/style.rs`, `shell/style_tests.rs`, `hosts.rs`, `hosts/display.rs`,
`settings.rs`, `settings/input_options.rs`, `settings/sidebar_density.rs`,
`settings_view.rs`, `app.rs`. A stream that needs a change there sends a
patch.

### 9.3 Shared values and types (Wave 0a)

- **`shell::style`** (`shell/style.rs`), every value cited to its artboard
  line and checked by `style_tests.rs`:
  - `color::{BACKGROUND, SURFACE, RAISED, SEPARATOR, MUTED, TEXT, ACCENT, GREEN, YELLOW, RED, SLATE, DISABLED}: u32`
    (`#101419 #1c1c22 #2b2933 #3b3945 #acb2bb #d8dee9 #88c0d0 #a3be8c #ebcb8b #bf616a #4c566a #6c7480`);
  - `pub(crate) struct Tint { rgb: u32, alpha: f32 }` with `From<Tint>` for
    `gpui::Rgba` and `gpui::Hsla`; `tint::{SELECTED, CURRENT_ROW, SELECTION, SHEET_SCRIM, POPOVER_SCRIM}`;
  - `pub(crate) struct Shadow { y: f32, blur: f32, alpha: f32 }`,
    `Shadow::box_shadow(self) -> gpui::BoxShadow`;
    `shadow::{SHEET, POPOVER, EDIT_BAR}`;
  - `radius::{SHEET, POPOVER, CARD, EDIT_BAR, CONTROL, RAIL_ITEM, BADGE}: f32`;
  - `text::{BASE, NAME, SECONDARY, SMALL, SHEET_TITLE, PAGE_TITLE, PAGE_TITLE_LINE, AGENT_LINE, HEADING_TRACKING_EM}: f32`
    (13, 14, 12, 11, 15, 18, 26, 16, 0.06; GPUI has no letter spacing, so
    the tracking is recorded, not drawn);
  - `weight::{REGULAR, MEDIUM, SEMIBOLD, BOLD}: gpui::FontWeight`;
  - `metric::{TOUCH, DOT, SMALL_DOT, RING, HEADING_ROW, HEADING_GAP, SIDE, ROW_GAP, INLINE_GAP, SPACE_INDENT, CURRENT_LINE, DETAIL_INDENT, LINE, PANE_NAME_INSET, MENU_WIDTH, MENU_ITEM, MENU_PADDING, SHEET_HEADER, SETTINGS_LIST, SETTINGS_ROW, SEGMENT, SEGMENT_INSET}: f32`.
  Region sizes stay in `crate::layout` and row heights in `shell::sidebar`.
- **Root text size**: `app.rs` sets `text_size(style::text::BASE)` on every
  root of `Smoke::render`.
- **`hosts::HostName`** (MAIN-20): `HostName::new(destination: &str) -> Self`,
  `destination(&self) -> &str`, `display(&self) -> &str`, and `Display`.
  The display name is the destination host's first DNS label (an IP stays
  whole; a string that is not a target shows as written). The destination
  stays the identity and the only thing SSH connects to; the display name is
  never saved or used as a key. A, B and C draw `display()` wherever a host
  name is shown. Until they do, `merge` fills `ListSpace::host` from
  `destination()` and `Display` formats through `display()`, so the iOS
  binary's `-D warnings` lint sees both in use without an allow.
- **`shell::sidebar_status`**: `SidebarStatus::dot_color()` follows
  SidebarSpec's legend (Needs you and Error red, Working yellow, Done green,
  Idle accent, Unknown and no status slate) and colours dots only (space,
  AGENTS and rail). `SidebarStatus::label_color()` colours the AGENTS status
  word and keeps its pre-Wave 0a colours (`#bf616a #88c0d0 #a3be8c #9ca3af
  #b48ead`), so no word loses contrast on the sidebar (the slate dot colour
  would draw Unknown at 2.44:1). Stream A restyles the words per
  Main.dc.html:94, :101, :108: Needs you `color::YELLOW` at `weight::BOLD`,
  every other word `color::MUTED` (11.55:1 and 8.45:1 on `#16161b`), and
  replaces `status_words_keep_their_colours_while_the_dots_change` with
  `agents_status_word_is_bold_yellow_needs_you_or_muted`.
  `pub(crate) enum AgentGroup { NeedsYou, Error, Pinned, Working, Done, Idle, Unknown, NoStatus }`
  and `AgentGroup::of(status: SidebarStatus, pinned: bool) -> AgentGroup`:
  the AGENTS order is the declaration order (owner decision: Needs you,
  Error, pinned, Working, Done, Idle). A pin never moves an agent out of
  Needs you or Error. Stream A replaces `status.rank()` in
  `agent_order_in` with `AgentGroup::of(row.status, pinned)`; without pins
  the two orders are the same (`without_pins_the_group_keeps_the_status_rank`).
- **`shell::sidebar_list`** (MIX-01, H2-07):
  `pub(crate) struct HostConnection { kind: HerdrKind, link: HostLink }`,
  `pub(crate) struct ListHost { name: HostName, connections: Vec<HostConnection>, selected: bool }`,
  `pub(crate) struct ListSpace { host: String, kind: HerdrKind, boot: String, generation: u64, entry: SpaceEntry }`,
  `ListSpace::tap(&self) -> SidebarTap`,
  `pub(crate) enum ListRow { Host(ListHost), Space(ListSpace) }`,
  `pub(crate) fn sources(host_entries: Vec<HostEntry>, herdr2_hosts: Vec<HostEntry>) -> Vec<HostEntry>`,
  and `pub(crate) fn merge(hosts: &[HostEntry]) -> Vec<ListRow>`: one host
  row per destination where its first entry stands, then its Herdr 1
  spaces, then its Herdr 2 sessions. Stream A draws the list from
  `merge(&sources(host_entries(..), herdr2_hosts(..)))`; stream F keeps
  producing Herdr 2 `HostEntry` values (`herdr2_frame::session`), one
  `SpaceEntry` per session, and does not edit `sidebar_list.rs`. The two
  contracts below are fixed by Wave 0a's tests:
  - *Connection identity.* Each `ListSpace` copies `boot` and `generation`
    from its `HostEntry` when the list is built, as the Herdr 1 space row
    does today (`sidebar::view::space_row`). A taps with `ListSpace::tap()`,
    which sends those drawn values; it never re-reads the current boot or
    generation at tap time. So a row drawn before a reconnect is rejected
    ("Stale board row", `board::navigate_board_inner`) even when the new
    connection has a space with the same ID:
    `a_row_drawn_before_a_reconnect_to_the_same_workspace_id_is_rejected`
    (`shell/sidebar_list_reconnect_tests.rs`).
  - *One source for Herdr 2 rows.* `herdr2_frame::sidebar::herdr2_hosts`
    (F) is the only source of Herdr 2 entries; `sidebar::view::host_entries`
    (A) also returns every Herdr 2 reader but without sessions, and
    `sources` replaces such an entry, where it stands, by the
    `herdr2_hosts` entry of the same destination. Neither A nor F
    concatenates the two lists (`merge` keeps the first entry of a kind,
    so the session-bearing row would be dropped):
    `a_herdr2_destination_from_both_sources_keeps_its_sessions`,
    `sources_keep_herdr1_entries_and_append_unmatched_herdr2_hosts`
    (`shell/sidebar_list_tests.rs`).
- **Sidebar density**: `settings::sidebar_density::SidebarDensity { Dense, Roomy }`
  (default Dense = the artboards' rows), stored as
  `"sidebar_density": "roomy"` through `input_options::FIELDS`
  (`SIDEBAR_DENSITY`); `settings::Values::sidebar_density`; the live value
  is `SidebarState::density`, set when the settings load and by
  `apply_settings`. Stream E adds the Settings › Sidebar page and calls
  `apply_settings` with the new value under the selection rule in 9.5; stream A draws by
  `self.shell.sidebar.density` and decides Roomy's extra room. Stream A
  also retires `board_density_toggle_removed` (`shell/sidebar_tests/rows.rs`).
- **`CoreStatus`**: the herdr-gpui pin is 9327f448, so
  `shell::herdr2_frame::CoreStatus` is `sheprd_core_client::CoreStatus`
  (1e105efaf). `CoreStatus::from(&Terminal)` replaces the removed
  `core_status`, and the status poll calls `CoreBackend::statuses`.

### 9.4 Tests each stream adds

Host tests named as below, in a new topic file under the module's
`*_tests/` directory, plus the Simulator screenshot of 9.1 rule 5.

- **A**: `sidebar_header_has_collapse_title_search_and_new_space`,
  `host_row_chevron_collapses_its_spaces`,
  `sidebar_draws_one_list_for_herdr1_and_herdr2`,
  `host_row_second_line_names_ssh_herdr_and_state`,
  `host_rows_show_the_display_name`,
  `connecting_host_dot_is_a_yellow_ring`,
  `space_row_shows_label_and_singular_tab_count`,
  `needs_you_badge_and_error_tag_follow_the_canvas`,
  `agents_list_puts_pinned_after_needs_you_and_error`,
  `agent_pin_survives_restart_and_drops_closed_panes`,
  `agent_row_shows_task_title_and_pane_name`,
  `agents_status_word_is_bold_yellow_needs_you_or_muted`,
  `space_tap_after_reconnect_uses_the_drawn_connection`,
  `sidebar_density_changes_row_heights`.
- **B**: `rail_marks_only_the_current_space`,
  `rail_space_dot_is_the_most_urgent_agent`,
  `rail_draws_a_rule_between_hosts`,
  `rail_labels_use_the_display_name`,
  `rail_footer_buttons_follow_the_canvas`,
  `rail_long_press_opens_the_row_menu`.
- **C**: `selected_tab_has_tint_underline_dot_and_semibold`,
  `tab_name_is_number_and_agent_or_process`,
  `new_tab_button_follows_the_last_tab`,
  `rail_mode_tab_row_shows_the_space_switcher`,
  `status_bar_shows_the_display_name_and_icons`,
  `selection_status_counts_characters_and_names_copy`,
  `edit_bar_has_shadow_medium_weight_and_cancel_icon`,
  `herdr2_paste_image_is_enabled`,
  `overflow_menu_matches_the_canvas_geometry`,
  `copy_as_one_line_follows_copy_in_the_edit_bar`,
  `copy_as_one_line_is_in_the_edit_menu_with_option_command_c`,
  `copy_as_one_line_is_not_in_the_overflow_menu`,
  `shortcuts_hint_only_with_a_hardware_keyboard`,
  `mods_item_has_icon_count_badge_and_label`; and
  `overflow_menu_closes_on_an_outside_press_that_stays_out_of_the_terminal`
  passes 20 runs in a row.
- **D**: `no_find_band_above_the_terminal_while_find_is_closed`,
  `terminal_padding_follows_the_canvas`,
  `selection_fill_is_the_canvas_tint`,
  `pane_borders_draw_the_name_above_a_one_point_border`,
  `copy_as_one_line_joins_wrapped_rows`,
  `option_command_c_copies_as_one_line`.
- **E**: `settings_header_is_saved_title_done`,
  `two_way_choices_are_segments`,
  `settings_sections_include_sidebar`,
  `sidebar_page_sets_the_density`,
  `density_chosen_before_load_survives_a_saved_roomy`,
  `add_host_header_buttons_are_text_buttons`,
  `device_key_is_read_when_the_sheet_opens`,
  `tapping_the_backdrop_closes_the_sheet`.
- **F**: `herdr2_lists_every_session`,
  `herdr2_tab_row_matches_herdr1`,
  `herdr2_agent_row_tap_opens_its_terminal`,
  `new_space_asks_for_the_host_and_the_herdr`,
  `herdr2_sessions_merge_into_the_sidebar_list` (through `sources`, with a
  Herdr 2 host that `host_entries` also returns).
- **H1**: `screen_stays_awake_while_connected`,
  `screen_may_sleep_after_the_last_disconnect`.

Wave 0a's own tests: `style_tests.rs` (the canvas values),
`sidebar_status_tests.rs` (the dot legend, the unchanged status-word
colours and their contrast, and `AgentGroup`),
`sidebar_list_tests.rs` (`merge` and `sources`),
`sidebar_list_reconnect_tests.rs` (a stale merged row after a reconnect), `hosts/display_tests.rs`
(`HostName`), `settings/sidebar_density_tests.rs` (storage and the live
value), and `herdr2_frame_tests/status.rs` (`CoreStatus::from`).

### 9.5 Copy as one line and the density choice (review of PR 90)

- **Copy as one line (owner decision 2026-11-20 17:0x, "編集バーとメニューバー").**
  Stream C places the item in two entries and nowhere else: the edit bar,
  as the button right after Copy (`shell/edit_bar.rs`), and the iPadOS menu
  bar's Edit menu as "Copy as One Line" with ⌥⌘C. It is not in the …
  menu (`shell/overflow_menu.rs`). The menu bar is registered in
  `native_menus.rs` (`Smoke::install_native_menus`, today a `Terminal`
  menu built from `actions::catalog` rows and the host menus), which C
  owns for this work. Native items are keyless (the label advertises the
  shortcut), so the ⌥⌘C key itself is handled next to ⌘C in
  `terminal_controls.rs`, D's selection and copy handling, and calls the
  same join D writes. C first confirms how the pinned GPUI turns a `Menu`
  titled `Edit` into the system Edit menu; if it cannot reach that menu,
  C stops and returns to the controller. Tests: C
  `copy_as_one_line_follows_copy_in_the_edit_bar`,
  `copy_as_one_line_is_in_the_edit_menu_with_option_command_c`,
  `copy_as_one_line_is_not_in_the_overflow_menu`; D
  `copy_as_one_line_joins_wrapped_rows`, `option_command_c_copies_as_one_line`.
- **Density selection before the settings load (stream E).** The Sidebar
  page may be shown before `SettingsHost` has loaded the file. A density
  the user picks then is explicit intent and the loaded value does not
  overwrite it, even when the pick equals the displayed default (Dense):
  the page calls a `SettingsHost::edit_sidebar_density(values)` that marks
  the `SIDEBAR_DENSITY` entry of `edited_before_load.choices` as edited
  regardless of a value change, as `edit_density` and `edit_font_family`
  already do (`EditIntent`). `settings.rs` is a Wave 0a file, so E sends
  that change as a patch in its PR. Gating the control on
  `settings.is_some()` alone (as `settings_sheet/view.rs` gates the other
  controls) is not enough: an unchanged Dense pick sets no flag and the
  saved Roomy would replace it. Test:
  `density_chosen_before_load_survives_a_saved_roomy` (file saves Roomy;
  before the load completes, Dense is picked; after the load the draft and
  the file say Dense).
