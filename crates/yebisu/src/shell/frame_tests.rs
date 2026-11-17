use super::*;
use crate::layout::{FIND_BAR_HEIGHT, TAB_ROW_HEIGHT};

/// The three canvases plan v2 7.5 names: 13-inch, 11-inch, Split View half.
const WINDOWS: [Size; 3] = [
    Size::new(1376.0, 1032.0),
    Size::new(1210.0, 834.0),
    Size::new(605.0, 834.0),
];

fn insets(keyboard: f32) -> Insets {
    Insets {
        safe_top: 24.0,
        safe_bottom: 20.0,
        keyboard,
        ..Insets::default()
    }
}

#[test]
fn find_bar_renders_at_terminal_column_top() {
    let tab = COLUMN_ORDER.iter().position(|s| *s == ColumnSlot::TabRow);
    let find = COLUMN_ORDER.iter().position(|s| *s == ColumnSlot::FindBar);
    let terminal = COLUMN_ORDER.iter().position(|s| *s == ColumnSlot::Terminal);
    assert_eq!((tab, find, terminal), (Some(0), Some(1), Some(2)));
    for window in WINDOWS {
        for drawn in [true, false] {
            let layout = layout::compute(&frame_input(window, insets(0.0), true));
            let g = frame_geometry(&layout, drawn);
            let Some(term) = g.terminal else {
                panic!("{window:?} fits");
            };
            assert_eq!(g.find_bar.height, FIND_BAR_HEIGHT);
            assert_eq!(g.find_bar.y, g.tab_row.bottom() + RULE, "{window:?}");
            assert_eq!(term.y, g.find_bar.bottom(), "{window:?}");
            assert_eq!((g.find_bar.x, g.find_bar.width), (term.x, term.width));
            // Closed: no height, the terminal starts under the tab row.
            let closed = layout::compute(&frame_input(window, insets(0.0), false));
            let g = frame_geometry(&closed, drawn);
            assert_eq!(g.find_bar.height, 0.0);
            assert_eq!(g.terminal.map(|r| r.y), Some(g.tab_row.bottom() + RULE));
        }
    }
}

#[test]
fn terminal_receives_allocated_rect() {
    for window in WINDOWS {
        for keyboard in [0.0, 320.0] {
            let layout = layout::compute(&frame_input(window, insets(keyboard), false));
            let Some(allocated) = layout.terminal.rect() else {
                panic!("{window:?} fits");
            };
            let g = frame_geometry(&layout, true);
            assert_eq!(g.terminal, Some(allocated), "{window:?} {keyboard}");
            assert_eq!(g.sidebar, Some(layout.sidebar));
            // The frame's fixed rows add up to the window: tab row, rule,
            // terminal, rule, status bar, then the bottom inset.
            let used = TAB_ROW_HEIGHT + RULE + allocated.height + RULE + g.status_bar.height;
            let bottom = keyboard.max(20.0);
            assert_eq!(24.0 + used + bottom, window.height, "{window:?} {keyboard}");
            // An empty sidebar column hands its width to the terminal.
            let wide = frame_geometry(&layout, false);
            let Some(rect) = wide.terminal else {
                panic!("{window:?} fits");
            };
            assert_eq!(rect.x, layout.sidebar.x);
            assert_eq!(rect.right(), allocated.right());
            assert_eq!((rect.y, rect.height), (allocated.y, allocated.height));
            assert_eq!(wide.sidebar, None);
        }
    }
}
