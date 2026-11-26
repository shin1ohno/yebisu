use super::super::{Input, Insets};
use super::{layout, terminal_size, window};

#[test]
fn keyboard_and_safe_area_take_larger_only() {
    let at = |insets: Insets| {
        layout(Input {
            insets,
            ..window(1000.0, 834.0)
        })
    };
    let bare = terminal_size(&at(Insets::default())).height;
    let keyboard = Insets {
        keyboard: 320.0,
        safe_bottom: 20.0,
        ..Insets::default()
    };
    assert_eq!(keyboard.bottom(), 320.0);
    assert_eq!(terminal_size(&at(keyboard)).height, bare - 320.0);
    let safe_only = Insets {
        safe_bottom: 20.0,
        ..Insets::default()
    };
    assert_eq!(terminal_size(&at(safe_only)).height, bare - 20.0);
    // The other edges are taken as they are.
    let edges = Insets {
        safe_top: 24.0,
        safe_left: 10.0,
        safe_right: 6.0,
        ..Insets::default()
    };
    let out = at(edges);
    assert_eq!(out.sidebar.x, 10.0);
    assert_eq!(out.sidebar.y, 24.0);
    assert_eq!(terminal_size(&out).height, bare - 24.0);
    assert_eq!(
        terminal_size(&out).width,
        terminal_size(&at(Insets::default())).width - 16.0
    );
}

#[test]
fn a_window_with_controls_gives_up_the_bottom_band_but_not_the_keyboard() {
    use super::super::bottom_band;
    // The owner's external-display window: 25 pt below the status bar.
    assert_eq!(bottom_band(25.0, true), 0.0);
    // Full screen and a Mac keep UIKit's value; a NaN or negative one is 0.
    assert_eq!(bottom_band(20.0, false), 20.0);
    assert_eq!(bottom_band(f32::NAN, false), 0.0);
    assert_eq!(bottom_band(-4.0, false), 0.0);
    // A docked keyboard still lifts the status bar in the window.
    let docked = Insets {
        safe_bottom: bottom_band(25.0, true),
        keyboard: 320.0,
        ..Insets::default()
    };
    assert_eq!(docked.bottom(), 320.0);
}
