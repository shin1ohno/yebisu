//! Shared fixtures: a Smoke in a test window and a real selection.
use crate::{app::Smoke, handoff::Selection};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use std::sync::Arc;

pub(super) fn window(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    (view, cx)
}

/// `日本語` on the mock surface, three characters, not dragging.
pub(super) fn selection() -> Selection {
    Selection {
        frame: Arc::new(crate::mock_peer::surface(7).frame),
        anchor: (1, 2),
        end: (6, 2),
        dragging: false,
    }
}
