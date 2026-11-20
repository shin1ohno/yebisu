//! L0 geometry tests, one file per rule family. Names follow plan-2 sections
//! 6.2 and 7.2 so the layout mutation check can name them.
use super::{Input, Insets, Layout, Regions, Size, compute};

mod artboards;
mod clamps;
mod controls;
mod find_bar;
mod insets;
mod minimum;
mod panes;
mod rail;
mod regions;
mod withdrawal;

/// Sidebar list content of both artboards (SplitHalf.dc.html: 564).
pub(super) const LIST_HEIGHT: f32 = 564.0;

/// A window with no insets, no mods, one pane and the find bar closed.
pub(super) fn window(width: f32, height: f32) -> Input {
    Input {
        window: Size::new(width, height),
        sidebar_list_height: LIST_HEIGHT,
        pane_count: 1,
        ..Input::default()
    }
}

/// Artboard 10: 11-inch, keyboard 320, every region at its maximum.
pub(super) fn artboard_10() -> Input {
    Input {
        insets: Insets {
            keyboard: 320.0,
            ..Insets::default()
        },
        regions: Regions::maximum(),
        pane_count: 2,
        ..window(1210.0, 834.0)
    }
}

/// Artboard 11: Split View half, M1 to M4 at their defaults.
pub(super) fn artboard_11() -> Input {
    Input {
        regions: Regions::defaults(),
        pane_count: 2,
        ..window(605.0, 834.0)
    }
}

pub(super) fn terminal_size(layout: &Layout) -> Size {
    match layout.terminal.rect() {
        Some(rect) => rect.size(),
        None => panic!("terminal does not fit: {:?}", layout.terminal),
    }
}

pub(super) fn layout(input: Input) -> Layout {
    compute(&input)
}
