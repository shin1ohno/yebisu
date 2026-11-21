//! L2c: one pane when the split would fall below the terminal minimum.
//!
//! Herdr 1: the daemon splits one size between the panes, so yebisu zooms the
//! focused pane of a tab while the column is narrower than two panes need,
//! and releases only the zoom it set itself once the column is wide again.
//! Herdr 2: yebisu composes the panes (`core/screen.rs`); this module hands
//! it the column's measurements through a latest-value mailbox.
use crate::{
    app::Smoke,
    input_geometry::InputGeometry,
    layout::{PaneMode, TERMINAL_MIN_HEIGHT, TERMINAL_MIN_WIDTH, pane_mode},
    shell::herdr2_frame::panes::pane_rects,
};
use gpui::{Bounds, Context, Point, Window, px, size};
use sheprd_client::protocol::ClientShellSnapshot;
use sheprd_commands::Command;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

#[path = "compact/mod.rs"]
#[allow(dead_code)] // Removed when K1's frame and sizing hooks are wired.
mod compact;

/// How long a sent Zoom or Release may stay unconfirmed before it is sent
/// again (a request the daemon dropped must not leave two panes in a narrow
/// column for good).
const REQUEST_TTL: Duration = Duration::from_secs(3);

/// Where a tab yebisu zoomed stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Zoom sent, the snapshot does not show it yet.
    Zooming,
    /// The snapshot shows the zoom yebisu set.
    Held,
    /// Release sent, the snapshot still shows the zoom.
    Releasing,
    /// Zoom sent, never shown within `REQUEST_TTL`: not sent again until the
    /// column crosses the split threshold or the tab's pane count changes
    /// (a repeat would discard input each time). Switching tabs does not lift it.
    GaveZoom,
    /// Release sent, the zoom stayed: not sent again until the column changes.
    GaveRelease,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SplitLayout {
    vertical: bool,
    /// Number of panes in a measured row or column.
    equal_panes: Option<usize>,
    minimum_width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Forced {
    phase: Phase,
    /// Pane count of the tab when the request went out: a split in between
    /// makes it a new situation.
    panes: usize,
    at: Instant,
    split: SplitLayout,
}

/// What the last frame showed, to record only changes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Shown {
    host: String,
    tab: u64,
    panes: usize,
}

/// Tabs yebisu zoomed itself, keyed by (host, tab id).
#[derive(Debug, Default)]
pub(crate) struct SinglePaneState {
    compact: compact::CompactState,
    forced: BTreeMap<(String, u64), Forced>,
    shown: Option<Shown>,
    /// A redraw was asked for because the column had no measured width yet.
    waiting_for_width: bool,
    /// The column Herdr 2 was last told about.
    published: Option<ColumnMetrics>,
    mailbox: Mailbox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomRequest {
    Zoom,
    Release,
}

impl SinglePaneState {
    /// A test's own mailbox, so it never reaches another test's worker.
    #[cfg(test)]
    pub(crate) fn set_mailbox(&mut self, mailbox: Mailbox) {
        self.mailbox = mailbox;
    }

    /// The mailbox this frame publishes its column to; the Herdr 2 worker
    /// it starts reads the same one.
    pub(crate) fn mailbox(&self) -> Mailbox {
        self.mailbox.clone()
    }

    fn entry(&self, host: &str, tab: u64) -> Option<&Forced> {
        self.forced.get(&(host.to_owned(), tab))
    }

    /// Folds what the snapshot shows into the record: a zoom that arrived is
    /// held, a zoom that is gone is forgotten, a request that was never
    /// answered is given up (and not repeated, see `Phase::GaveZoom`).
    pub(crate) fn observe(&mut self, host: &str, tab: u64, zoomed: bool, now: Instant) {
        let key = (host.to_owned(), tab);
        let Some(entry) = self.forced.get_mut(&key) else {
            return;
        };
        let stale = now.saturating_duration_since(entry.at) >= REQUEST_TTL;
        match (entry.phase, zoomed) {
            (Phase::Zooming | Phase::GaveZoom, true) => entry.phase = Phase::Held,
            (Phase::Zooming, false) if stale => entry.phase = Phase::GaveZoom,
            (Phase::Held | Phase::Releasing | Phase::GaveRelease, false) => {
                self.forced.remove(&key);
            }
            (Phase::Releasing, true) if stale => entry.phase = Phase::GaveRelease,
            _ => {}
        }
    }

    /// The column crossed the split threshold for this tab: a request given up on
    /// the other side may be sent again.
    pub(crate) fn column_changed(&mut self, host: &str, tab: u64, narrow: bool) {
        let key = (host.to_owned(), tab);
        match self.forced.get_mut(&key) {
            Some(entry) if narrow && entry.phase == Phase::GaveRelease => entry.phase = Phase::Held,
            Some(entry) if !narrow && entry.phase == Phase::GaveZoom => {
                self.forced.remove(&key);
            }
            _ => {}
        }
    }

    /// Forgets tabs of `host` that the snapshot no longer lists.
    pub(crate) fn retain_tabs(&mut self, host: &str, tabs: &[u64]) {
        self.forced
            .retain(|(h, tab), _| h != host || tabs.contains(tab));
    }

    /// Notes that a Zoom or Release went out for a tab with `panes` panes.
    fn record_sent(
        &mut self,
        host: &str,
        tab: u64,
        request: ZoomRequest,
        panes: usize,
        now: Instant,
        split: SplitLayout,
    ) {
        let phase = match request {
            ZoomRequest::Zoom => Phase::Zooming,
            ZoomRequest::Release => Phase::Releasing,
        };
        self.forced.insert(
            (host.to_owned(), tab),
            Forced {
                phase,
                panes,
                at: now,
                split,
            },
        );
    }
}

/// Herdr 1: what to send for one tab, given its pane budget and zoom state.
///
/// - Narrow column, two or more panes, not zoomed: `Zoom`, once (a request
///   already out, or given up on, for this pane count is not repeated).
/// - Wide column, zoomed, and the zoom is the one yebisu set: `Release`,
///   once. A zoom the user set stays.
pub(crate) fn herdr1_zoom_request(
    state: &SinglePaneState,
    host: &str,
    tab: u64,
    mode: PaneMode,
    pane_count: usize,
    zoomed: bool,
) -> Option<ZoomRequest> {
    let entry = state.entry(host, tab);
    match mode {
        PaneMode::Single if pane_count >= 2 => {
            let asked = entry.is_some_and(|e| {
                matches!(e.phase, Phase::Zooming | Phase::GaveZoom) && e.panes == pane_count
            });
            (!zoomed && !asked).then_some(ZoomRequest::Zoom)
        }
        PaneMode::Single => None,
        PaneMode::Split => {
            let ours = entry.is_some_and(|e| e.phase == Phase::Held);
            (zoomed && ours).then_some(ZoomRequest::Release)
        }
    }
}

/// The focused tab of a snapshot, as the rule needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TabView {
    pub(crate) key: u64,
    pub(crate) panes: usize,
    pub(crate) zoomed: bool,
}

/// FNV-1a of the daemon's tab id. The contract's `tab: u64` cannot hold the
/// id string, and a tab's `number` repeats across spaces.
pub(crate) fn tab_key(tab_id: &str) -> u64 {
    tab_id.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub(crate) fn focused_tab(snapshot: &ClientShellSnapshot) -> Option<TabView> {
    let id = snapshot.focused_tab_id.as_ref()?;
    let tab = snapshot.tabs.iter().find(|tab| &tab.tab_id == id)?;
    let panes = snapshot
        .panes
        .iter()
        .filter(|pane| pane.tab_id == tab.tab_id && pane.workspace_id == tab.workspace_id)
        .count();
    Some(TabView {
        key: tab_key(&tab.tab_id),
        panes,
        zoomed: tab.zoomed,
    })
}

pub(crate) fn tab_keys(snapshot: &ClientShellSnapshot) -> Vec<u64> {
    snapshot
        .tabs
        .iter()
        .map(|tab| tab_key(&tab.tab_id))
        .collect()
}

/// The terminal column's measurements, in points. Written by the UI each
/// frame, read by the Herdr 2 worker when it sizes and composes the panes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ColumnMetrics {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) cell_width: f32,
    pub(crate) cell_height: f32,
    pub(crate) focus_only: bool,
}

impl ColumnMetrics {
    pub(crate) fn equal_panes_fit(&self, count: usize, vertical: bool) -> bool {
        let geometry = InputGeometry::from_canvas(
            Bounds::new(Point::default(), size(px(self.width), px(self.height))),
            self.cell_width,
            self.cell_height,
        );
        let budget = geometry.equal_pane_viewport(count, vertical);
        let budget = crate::terminal::viewport(
            budget.width.into(),
            budget.height.into(),
            self.cell_width,
            self.cell_height,
        );
        let panes = pane_rects(budget.cols, budget.rows, count, vertical);
        let (least_cols, least_rows) = self.minimum_cells();
        panes.len() >= count
            && panes.first().is_some_and(|pane| {
                if vertical {
                    pane.rows >= least_rows
                } else {
                    pane.cols >= least_cols
                }
            })
    }

    /// The least columns and rows one pane may be given.
    pub(crate) fn minimum_cells(&self) -> (u16, u16) {
        let cells = |points: f32, cell: f32| {
            if cell > 0.0 {
                (points / cell).ceil().clamp(1.0, f32::from(u16::MAX)) as u16
            } else {
                1
            }
        };
        (
            cells(TERMINAL_MIN_WIDTH, self.cell_width),
            cells(TERMINAL_MIN_HEIGHT, self.cell_height),
        )
    }
}

/// The latest column, shared between the UI thread (writer) and the Herdr 2
/// worker (reader). The view hands its mailbox to the worker it starts
/// (`Session::start`), and the worker reads only that one. Production code
/// uses the one process-wide mailbox; a test builds its own with
/// [`Mailbox::detached`] so it never leaks into another test's worker.
#[derive(Clone, Debug)]
pub(crate) struct Mailbox(Arc<Mutex<Option<ColumnMetrics>>>);

impl Mailbox {
    fn global() -> Self {
        static GLOBAL: OnceLock<Mailbox> = OnceLock::new();
        GLOBAL.get_or_init(Self::detached).clone()
    }

    pub(crate) fn detached() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub(crate) fn publish(&self, metrics: Option<ColumnMetrics>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = metrics;
    }

    pub(crate) fn latest(&self) -> Option<ColumnMetrics> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::global()
    }
}

impl Smoke {
    /// Called once per terminal frame before the screen is built.
    pub(crate) fn enforce_pane_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_compact_panels(window);
        let grid = self.terminal.column_bounds().size;
        let (width, height) = (f32::from(grid.width), f32::from(grid.height));
        if !self.terminal.open || width <= 0.0 {
            // No terminal column has been measured yet; the paint that
            // measures it does not redraw by itself.
            if self.terminal.open && !self.shell.single_pane.waiting_for_width {
                self.shell.single_pane.waiting_for_width = true;
                cx.defer_in(window, |_, _, cx| cx.notify());
            }
            return;
        }
        self.shell.single_pane.waiting_for_width = false;
        if self.terminal.core_mode {
            self.publish_core_column(ColumnMetrics {
                width,
                height,
                cell_width: self.terminal.geometry.cell_width,
                cell_height: self.terminal.geometry.cell_height,
                focus_only: self.pager_available_at(window),
            });
            return;
        }
        if self.shell.single_pane.published.take().is_some() {
            self.shell.single_pane.mailbox.publish(None);
        }
        self.enforce_herdr1(width);
    }

    /// Tells the Herdr 2 worker about the column. The worker only sizes its
    /// terminals when it is sent a command, and the Resize of this frame was
    /// sent from paint with the previous column, so a changed column forgets
    /// the size that was sent: the next paint sends it again.
    fn publish_core_column(&mut self, metrics: ColumnMetrics) {
        let state = &mut self.shell.single_pane;
        if state.published == Some(metrics) {
            return;
        }
        state.published = Some(metrics);
        state.mailbox.publish(Some(metrics));
        self.terminal.size = None;
    }

    fn enforce_herdr1(&mut self, column_width: f32) {
        let host = self.terminal.reconnect.target.clone();
        let Some(snapshot) = crate::board::snapshot(&self.terminal.live) else {
            return;
        };
        let (view, tabs) = (focused_tab(snapshot), tab_keys(snapshot));
        let Some(view) = view else {
            return;
        };
        let now = Instant::now();
        let state = &mut self.shell.single_pane;
        state.retain_tabs(&host, &tabs);
        state.observe(&host, view.key, view.zoomed, now);
        let held_split = state
            .entry(&host, view.key)
            .filter(|entry| entry.panes == view.panes)
            .map_or(SplitLayout::default(), |entry| entry.split);
        let split = self.herdr1_split_layout(held_split);
        let geometry = self.terminal.geometry;
        let metrics = ColumnMetrics {
            width: column_width,
            height: self.terminal.column_bounds().size.height.into(),
            cell_width: geometry.cell_width,
            cell_height: geometry.cell_height,
            focus_only: false,
        };
        // A tab can list panes before the surface provides their rectangles.
        // Start with the two-pane budget and use actual widths for mixed splits.
        let equal_panes = split.equal_panes.unwrap_or(2);
        let mode = if pane_mode(column_width, view.panes) == PaneMode::Split
            && column_width >= split.minimum_width
            && metrics.equal_panes_fit(equal_panes, split.vertical)
        {
            PaneMode::Split
        } else {
            PaneMode::Single
        };
        self.shell
            .single_pane
            .column_changed(&host, view.key, mode == PaneMode::Single);
        self.record_layout(&host, view, column_width, mode);
        if self.auto_zoom_would_discard_input() {
            return;
        }
        let Some(request) = herdr1_zoom_request(
            &self.shell.single_pane,
            &host,
            view.key,
            mode,
            view.panes,
            view.zoomed,
        ) else {
            return;
        };
        let available = crate::actions::catalog(&self.terminal)
            .iter()
            .any(|a| a.info.command == Command::Zoom && a.available);
        if available && self.run_action(Command::Zoom).is_ok() {
            self.shell
                .single_pane
                .record_sent(&host, view.key, request, view.panes, now, split);
        }
    }

    fn herdr1_split_layout(&self, held: SplitLayout) -> SplitLayout {
        let Some((_, surface)) = self.terminal.coherent_frame() else {
            return held;
        };
        let [first, _, ..] = surface.panes.as_slice() else {
            return held;
        };
        let vertical = surface
            .panes
            .iter()
            .all(|pane| pane.rect.x == first.rect.x && pane.rect.width == first.rect.width);
        let horizontal = surface
            .panes
            .iter()
            .all(|pane| pane.rect.y == first.rect.y && pane.rect.height == first.rect.height);
        SplitLayout {
            vertical,
            equal_panes: (vertical || horizontal).then_some(surface.panes.len()),
            minimum_width: self
                .terminal
                .geometry
                .minimum_split_width(&surface, TERMINAL_MIN_WIDTH),
        }
    }

    /// `run_action` cancels a selection, closes the palette and discards typed
    /// and composing input, so the automatic Zoom waits while any of that is
    /// in use. It is sent on the first frame after.
    fn auto_zoom_would_discard_input(&self) -> bool {
        self.handoff.select_mode
            || self.handoff.selection.is_some()
            // Settings or the Hosts sheet over the terminal (L2c patch 03, A6).
            || self.input_modal()
            || self.find.is_some()
            || self.actions.palette
            // The Shortcuts popover holds the input too (review adf0 SF2).
            || self.shell.shortcuts.open
            || !self.terminal.input.composition.text.is_empty()
            // Keys typed while the surface is still at the old size wait in
            // the input queue; `detach` would drop them.
            || !self.terminal.input.ready
    }

    /// Records how many panes the screen shows, when that changes.
    fn record_layout(&mut self, host: &str, view: TabView, column_width: f32, mode: PaneMode) {
        let shown = match mode {
            PaneMode::Single => 1,
            PaneMode::Split => view.panes,
        };
        let now = Shown {
            host: host.to_owned(),
            tab: view.key,
            panes: shown,
        };
        if self.shell.single_pane.shown.as_ref() != Some(&now) {
            self.observation.record(serde_json::json!({
                "kind": "layout",
                "panes": shown,
                "tab_panes": view.panes,
                "column": column_width.round(),
            }));
            self.shell.single_pane.shown = Some(now);
        }
    }
}

#[cfg(test)]
#[path = "single_pane_tests.rs"]
mod tests;
