//! Issue #137 PR 6: a Herdr 2 pane publishes where it is in its history and
//! whether its program reads the mouse; it has no daemon-drawn bar column.
use super::support::{saved, screen, size};
use crate::core::screen::publish;
use sheprd_backend::{ScrollPosition, SurfaceUpdate};
use sheprd_client::protocol::PaneSurfaceScrollMetrics;

#[test]
fn a_pane_publishes_its_scroll_position_and_mouse_reading() -> crate::Result<()> {
    let size = size(90, 26)?;
    let layout = saved(0, false)?;
    let mut screens = [screen("a", 90, 26), screen("b", 90, 26)];
    let live = publish(&screens, &layout, size, 1, 1, 1, None);
    let surface = live.surface.as_ref().unwrap_or_else(|| panic!("surface"));
    assert!(
        surface.panes.iter().all(|pane| pane.scroll.is_none()),
        "untold"
    );
    screens[1].apply(SurfaceUpdate::Scroll(ScrollPosition {
        offset: 3,
        history: 100,
        rows: 24,
    }));
    screens[1].apply(SurfaceUpdate::MouseReporting(true));
    let live = publish(&screens, &layout, size, 1, 2, 1, None);
    let surface = live.surface.as_ref().unwrap_or_else(|| panic!("surface"));
    assert_eq!(surface.panes.len(), 2);
    let (first, second) = (&surface.panes[0], &surface.panes[1]);
    assert_eq!(first.scroll, None);
    assert!(!first.mouse_reporting);
    assert_eq!(
        second.scroll,
        Some(PaneSurfaceScrollMetrics {
            offset_from_bottom: 3,
            max_offset_from_bottom: 100,
            viewport_rows: 24,
        })
    );
    assert!(second.mouse_reporting);
    // The thumb rides the pane's last column; nothing masks its cells.
    assert_eq!(second.scrollbar_rect, None);
    screens[1].apply(SurfaceUpdate::MouseReporting(false));
    let live = publish(&screens, &layout, size, 1, 3, 1, None);
    let surface = live.surface.as_ref().unwrap_or_else(|| panic!("surface"));
    assert!(!surface.panes[1].mouse_reporting);
    Ok(())
}
