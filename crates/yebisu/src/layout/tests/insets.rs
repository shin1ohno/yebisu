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
