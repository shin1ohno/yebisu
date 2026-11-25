//! The full sidebar's footer is as tall as the status bar beside it, rule
//! included, so their top rules line up (owner decision 2026-11-25), and
//! the terminal column keeps its height.
use super::super::{Insets, RULE, SIDEBAR_FOOTER_HEIGHT, STATUS_BAR_HEIGHT, TAB_ROW_HEIGHT};
use super::{Input, artboard_10, artboard_11, layout, window};

#[test]
fn the_footer_is_one_rule_and_one_status_bar_tall() {
    assert_eq!(SIDEBAR_FOOTER_HEIGHT, 45.0);
    assert_eq!(SIDEBAR_FOOTER_HEIGHT, RULE + STATUS_BAR_HEIGHT);
}

#[test]
fn the_footer_rule_lines_up_with_the_status_bar_rule() {
    let inputs = [
        window(1000.0, 834.0),
        window(1376.0, 1032.0),
        artboard_10(),
        artboard_11(),
        // A bottom safe area lifts both bands together.
        Input {
            insets: Insets {
                safe_bottom: 20.0,
                ..Insets::default()
            },
            ..window(1000.0, 834.0)
        },
    ];
    for input in inputs {
        let out = layout(input);
        let footer_rule = out.sidebar.y + out.sidebar.height - SIDEBAR_FOOTER_HEIGHT;
        let status_rule = out.status_bar.y - RULE;
        assert_eq!(footer_rule, status_rule, "{:?}", input.window);
    }
}

#[test]
fn the_terminal_column_keeps_its_height() {
    // Tab row, its rule, the content, the status bar's rule and the status
    // bar fill the window; the sidebar footer takes nothing from them.
    let out = layout(window(1000.0, 834.0));
    let content = 834.0 - TAB_ROW_HEIGHT - RULE - RULE - STATUS_BAR_HEIGHT;
    let terminal = out
        .terminal
        .rect()
        .unwrap_or_else(|| panic!("terminal fits"));
    assert_eq!(terminal.y, TAB_ROW_HEIGHT + RULE);
    assert_eq!(
        terminal.y + terminal.height,
        TAB_ROW_HEIGHT + RULE + content
    );
}
