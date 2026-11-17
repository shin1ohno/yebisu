//! L2c: one pane below a 568 pt terminal column (plan v2 6). Owner: L2c.
use crate::app::Smoke;
use gpui::{Context, Window};

/// Tabs yebisu zoomed itself, keyed by (host, tab id).
#[derive(Debug, Default)]
pub(crate) struct SinglePaneState {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomRequest {
    Zoom,
    Release,
}

/// Herdr 1: what to send for one tab, given the column width, its pane
/// count and whether it is zoomed (L2c calls `layout::pane_mode`).
pub(crate) fn herdr1_zoom_request(
    _state: &SinglePaneState,
    _host: &str,
    _tab: u64,
    _column_width: f32,
    _pane_count: usize,
    _zoomed: bool,
) -> Option<ZoomRequest> {
    None
}

impl Smoke {
    /// Called once per terminal frame before the screen is built.
    pub(crate) fn enforce_pane_mode(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}
