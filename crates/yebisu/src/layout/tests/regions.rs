use super::super::{Input, ModPresence, ModSlot, Rect, Region, Regions, Size};
use super::{artboard_10, artboard_11, layout, terminal_size, window};

#[test]
fn dock_spans_bands_and_bands_span_terminal_column() {
    let out = layout(artboard_10());
    let dock = out.m2.unwrap_or_else(|| panic!("M2 shown"));
    let m3 = out.m3.unwrap_or_else(|| panic!("M3 shown"));
    let terminal = out
        .terminal
        .rect()
        .unwrap_or_else(|| panic!("terminal fits"));
    // M2 runs from under the tab row to over the status bar.
    assert_eq!(dock.y, out.tab_row.bottom() + 1.0);
    assert_eq!(dock.bottom(), out.status_bar.y - 1.0);
    assert_eq!(dock.x, terminal.right());
    // M3 and M4 are only as wide as the terminal column.
    assert_eq!((m3.x, m3.width), (terminal.x, terminal.width));
    let wide = layout(Input {
        regions: Regions {
            m4: ModSlot::Wants(44.0),
            ..Regions::maximum()
        },
        ..window(1376.0, 1032.0)
    });
    let m4 = wide.m4.unwrap_or_else(|| panic!("M4 shown"));
    let terminal = wide
        .terminal
        .rect()
        .unwrap_or_else(|| panic!("terminal fits"));
    assert_eq!((m4.x, m4.width), (terminal.x, terminal.width));
    let dock = wide.m2.unwrap_or_else(|| panic!("M2 shown"));
    let m3 = wide.m3.unwrap_or_else(|| panic!("M3 shown"));
    assert_eq!((dock.y, dock.bottom()), (m4.y, m3.bottom()));
    // The tab row and the status bar run over the column and the dock.
    assert_eq!(wide.tab_row.right(), dock.right());
    assert_eq!(wide.status_bar.right(), dock.right());
}

#[test]
fn empty_region_has_zero_size() {
    let out = layout(window(1000.0, 834.0));
    for region in [Region::M1, Region::M2, Region::M3, Region::M4] {
        assert_eq!(out.region_size(region), Size::default(), "{region:?}");
    }
    assert_eq!(out.m5.width, 0.0);
    assert_eq!(out.m6, None);
    assert!(out.withdrawn.is_empty());
}

#[test]
fn empty_region_reserves_nothing() {
    let out = layout(window(1000.0, 834.0));
    // 1000 − sidebar 300 − rule 1; 834 − tab row 37 − status bar 45.
    assert_eq!(terminal_size(&out), Size::new(699.0, 752.0));
    // M6 is the pane itself: present or not, the terminal is the same.
    let framed = layout(Input {
        regions: Regions {
            m6: ModPresence::Present,
            ..Regions::default()
        },
        ..window(1000.0, 834.0)
    });
    assert_eq!(framed.terminal, out.terminal);
    assert_eq!(framed.m6, out.terminal.rect());
}

#[test]
fn mismatched_mod_region_is_absent() {
    let mismatched = Regions {
        m1: ModPresence::VersionMismatch,
        m2: ModSlot::VersionMismatch,
        m3: ModSlot::VersionMismatch,
        m4: ModSlot::VersionMismatch,
        m5_sections: 0,
        m6: ModPresence::VersionMismatch,
    };
    let empty = layout(window(605.0, 834.0));
    let out = layout(Input {
        regions: mismatched,
        ..window(605.0, 834.0)
    });
    assert_eq!(out, empty);
    assert!(out.withdrawn.is_empty());
    // Mixed: a mismatched M2 does not count against the others' room.
    let mixed = layout(Input {
        regions: Regions {
            m2: ModSlot::VersionMismatch,
            ..Regions::defaults()
        },
        ..window(1000.0, 834.0)
    });
    assert_eq!(mixed.m2, None);
    assert!(!mixed.withdrawn.contains(&Region::M2));
    assert_eq!(terminal_size(&mixed).width, 699.0);
}

#[test]
fn qa_strip_is_outside_terminal_rect() {
    let product = layout(artboard_11());
    let qa = layout(Input {
        qa_strip_height: 40.0,
        ..artboard_11()
    });
    let terminal = qa
        .terminal
        .rect()
        .unwrap_or_else(|| panic!("terminal fits"));
    assert_eq!(qa.qa_strip.height, 40.0);
    assert!(!qa.qa_strip.overlaps(terminal));
    // Bottom of the column, above the status bar.
    assert_eq!(qa.qa_strip.bottom(), qa.status_bar.y - 1.0);
    assert_eq!(
        (qa.qa_strip.x, qa.qa_strip.width),
        (terminal.x, terminal.width)
    );
    // The only difference from the product build is the strip's height.
    assert_eq!(terminal_size(&product).height - terminal.height, 40.0);
    assert_eq!(product.qa_strip.height, 0.0);
    assert_eq!(
        qa.m3,
        product.m3.map(|m3| Rect {
            y: m3.y - 40.0,
            ..m3
        })
    );
}

#[test]
fn overlay_shrinks_to_window_less_32() {
    let roomy = layout(artboard_11()).overlay;
    assert_eq!(
        (roomy.dialog_width, roomy.toast),
        (420.0, Size::new(340.0, 72.0))
    );
    assert_eq!(roomy.max_height, 834.0 - 32.0);
    let tight = layout(window(300.0, 500.0)).overlay;
    assert_eq!(
        (tight.dialog_width, tight.toast),
        (268.0, Size::new(268.0, 72.0))
    );
    assert_eq!(tight.max_height, 468.0);
}

#[test]
fn status_sections_move_to_details_when_narrow() {
    let with_sections = |reserved| {
        layout(Input {
            regions: Regions {
                m5_sections: 3,
                ..Regions::defaults()
            },
            status_reserved_width: reserved,
            ..artboard_11()
        })
        .m5
    };
    // Status bar 304 wide (M2 withdrawn): one section of 160 fits.
    let one = with_sections(0.0);
    assert_eq!((one.shown, one.moved_to_details, one.width), (1, 2, 160.0));
    let none = with_sections(200.0);
    assert_eq!((none.shown, none.moved_to_details, none.width), (0, 3, 0.0));
}
