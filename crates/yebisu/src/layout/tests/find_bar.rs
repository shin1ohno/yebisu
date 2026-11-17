use super::super::{FIND_BAR_HEIGHT, Input, ModSlot, Region, Regions, TerminalFit};
use super::{artboard_10, artboard_11, layout, terminal_size, window};

#[test]
fn find_bar_sits_at_top_of_terminal_column() {
    let out = layout(Input {
        find_bar_open: true,
        ..artboard_11()
    });
    assert_eq!(FIND_BAR_HEIGHT, 44.0);
    assert_eq!(out.find_bar.height, 44.0);
    // Directly under the tab row and its rule, across the terminal column.
    assert_eq!(out.find_bar.y, out.tab_row.bottom() + 1.0);
    assert_eq!(out.find_bar.x, out.sidebar.right() + 1.0);
    assert_eq!(out.find_bar.width, terminal_size(&out).width);
    // M4 comes below it, then the terminal.
    let m4 = out.m4.unwrap_or_else(|| panic!("M4 shown"));
    assert_eq!(m4.y, out.find_bar.bottom());
    let terminal = out
        .terminal
        .rect()
        .unwrap_or_else(|| panic!("terminal fits"));
    assert_eq!(terminal.y, m4.bottom());
    // The 44 comes out of the terminal: 596 − 44.
    assert_eq!(terminal.height, 552.0);
}

#[test]
fn closed_find_bar_takes_no_height() {
    let closed = layout(artboard_11());
    assert_eq!(closed.find_bar.height, 0.0);
    let m4 = closed.m4.unwrap_or_else(|| panic!("M4 shown"));
    assert_eq!(m4.y, closed.tab_row.bottom() + 1.0);
    assert_eq!(terminal_size(&closed).height, 596.0);
}

#[test]
fn find_bar_open_withdraws_mods_before_terminal() {
    // Artboard 10 with the find bar: 432 − 44 = 388 for M4, M3 and the
    // terminal. M4 goes first, M3 takes 388 − 206 = 182, the terminal keeps 206.
    let out = layout(Input {
        find_bar_open: true,
        ..artboard_10()
    });
    assert_eq!(terminal_size(&out).height, 206.0);
    assert_eq!(out.region_size(Region::M3).height, 182.0);
    assert_eq!(out.m4, None);

    // Too short for M3's minimum as well: both bands go, the terminal keeps
    // all the column under the find bar.
    let short = layout(Input {
        find_bar_open: true,
        regions: Regions {
            m3: ModSlot::Wants(112.0),
            m4: ModSlot::Wants(44.0),
            ..Regions::default()
        },
        ..window(800.0, 82.0 + 44.0 + 206.0 + 43.0)
    });
    assert_eq!(terminal_size(&short).height, 249.0);
    assert_eq!(short.withdrawn, vec![Region::M3, Region::M4]);

    // One point short of 44 + 206 even with no bands: reported, not shrunk.
    let none = layout(Input {
        find_bar_open: true,
        ..window(800.0, 82.0 + 44.0 + 205.0)
    });
    assert!(matches!(none.terminal, TerminalFit::TooSmall(_)));
}
