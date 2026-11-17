//! L2a: the 36 pt tab row. Owner: L2a.
use crate::app::Smoke;
use gpui::{AnyElement, Context};

/// Tab row metrics (plan v2 7.5 `tab_row_metrics`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TabRowMetrics {
    pub(crate) height: f32,
    pub(crate) close_width: f32,
    pub(crate) close_height: f32,
}

/// What a tap on the tab row hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabHit {
    Select(usize),
    Close(usize),
    NewTab,
    Split,
    Overflow,
}

pub(crate) fn tab_row_metrics() -> TabRowMetrics {
    TabRowMetrics::default()
}

/// Hit test for a tap at `x` given each tab's width and the row width; the
/// pinned actions (+, split, ...) stay at the right edge.
pub(crate) fn tab_hit(_x: f32, _tab_widths: &[f32], _row_width: f32) -> Option<TabHit> {
    None
}

impl Smoke {
    pub(crate) fn render_tab_row(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}
