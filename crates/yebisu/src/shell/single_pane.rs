//! L2c: one pane below a 568 pt terminal column (plan v2 6). Owner: L2c.
//!
//! Herdr 1: the daemon splits one size between the panes, so yebisu zooms the
//! focused pane of a tab while the column is narrower than two panes need,
//! and releases only the zoom it set itself once the column is wide again.
//! Herdr 2: yebisu composes the panes (`core/screen.rs`); this module hands
//! it the column's measurements through a latest-value mailbox.
use crate::{
    app::Smoke,
    layout::{PaneMode, TERMINAL_MIN_HEIGHT, TERMINAL_MIN_WIDTH, pane_mode},
};
use gpui::{Context, Window};
use sheprd_client::protocol::ClientShellSnapshot;
use sheprd_commands::Command;
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Forced {
    phase: Phase,
    /// Pane count of the tab when the request went out: a split in between
    /// makes it a new situation.
    panes: usize,
    at: Instant,
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
    forced: BTreeMap<(String, u64), Forced>,
    shown: Option<Shown>,
    /// A redraw was asked for because the column had no measured width yet.
    waiting_for_width: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomRequest {
    Zoom,
    Release,
}

impl SinglePaneState {
    fn entry(&self, host: &str, tab: u64) -> Option<&Forced> {
        self.forced.get(&(host.to_owned(), tab))
    }

    /// Folds what the snapshot shows into the record: a zoom that arrived is
    /// held, a zoom that is gone is forgotten, a request that was never
    /// answered is dropped so the rule can ask again.
    pub(crate) fn observe(&mut self, host: &str, tab: u64, zoomed: bool, now: Instant) {
        let key = (host.to_owned(), tab);
        let Some(entry) = self.forced.get_mut(&key) else {
            return;
        };
        let stale = now.saturating_duration_since(entry.at) >= REQUEST_TTL;
        match (entry.phase, zoomed) {
            (Phase::Zooming, true) => entry.phase = Phase::Held,
            (Phase::Zooming, false) if stale => {
                self.forced.remove(&key);
            }
            (Phase::Held | Phase::Releasing, false) => {
                self.forced.remove(&key);
            }
            (Phase::Releasing, true) if stale => entry.phase = Phase::Held,
            _ => {}
        }
    }

    /// Forgets tabs of `host` that the snapshot no longer lists.
    pub(crate) fn retain_tabs(&mut self, host: &str, tabs: &[u64]) {
        self.forced
            .retain(|(h, tab), _| h != host || tabs.contains(tab));
    }

    /// Notes that a Zoom or Release went out for a tab with `panes` panes.
    pub(crate) fn record_sent(
        &mut self,
        host: &str,
        tab: u64,
        request: ZoomRequest,
        panes: usize,
        now: Instant,
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
            },
        );
    }
}

/// Herdr 1: what to send for one tab, given the column width, its pane
/// count and whether it is zoomed (L2c calls `layout::pane_mode`).
///
/// - Narrow column, two or more panes, not zoomed: `Zoom`, once (a request
///   already out for this pane count is not repeated).
/// - Wide column, zoomed, and the zoom is the one yebisu set: `Release`,
///   once. A zoom the user set stays.
pub(crate) fn herdr1_zoom_request(
    state: &SinglePaneState,
    host: &str,
    tab: u64,
    column_width: f32,
    pane_count: usize,
    zoomed: bool,
) -> Option<ZoomRequest> {
    let entry = state.entry(host, tab);
    match pane_mode(column_width, pane_count) {
        PaneMode::Single if pane_count >= 2 => {
            let asked = entry.is_some_and(|e| e.phase == Phase::Zooming && e.panes == pane_count);
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
}

impl ColumnMetrics {
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

static METRICS: Mutex<Option<ColumnMetrics>> = Mutex::new(None);

pub(crate) fn publish_column_metrics(metrics: Option<ColumnMetrics>) {
    let mut slot = METRICS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *slot = metrics;
}

/// The last published column, `None` until the first frame is measured.
pub(crate) fn column_metrics() -> Option<ColumnMetrics> {
    *METRICS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Smoke {
    /// Called once per terminal frame before the screen is built.
    pub(crate) fn enforce_pane_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let grid = self.terminal.geometry.grid.size;
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
            publish_column_metrics(Some(ColumnMetrics {
                width,
                height,
                cell_width: self.terminal.geometry.cell_width,
                cell_height: self.terminal.geometry.cell_height,
            }));
            return;
        }
        publish_column_metrics(None);
        self.enforce_herdr1(width);
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
        self.record_layout(&host, view, column_width);
        // A zoom changes the selection's pane; wait for the user to finish.
        if self.handoff.select_mode || self.handoff.selection.is_some() || self.settings_modal {
            return;
        }
        let Some(request) = herdr1_zoom_request(
            &self.shell.single_pane,
            &host,
            view.key,
            column_width,
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
                .record_sent(&host, view.key, request, view.panes, now);
        }
    }

    /// Records how many panes the screen shows, when that changes.
    fn record_layout(&mut self, host: &str, view: TabView, column_width: f32) {
        let shown = match pane_mode(column_width, view.panes) {
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
