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
above and below where they differ.

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
| L6 | `shell/shortcuts.rs`, `shell/host_menus.rs`, `native_menus.rs`, `native_menus_tests.rs`, `scripts/ios/p12-mutations.py` |
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
