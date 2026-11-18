use super::super::{Input, ModSlot, RAIL_WIDTH, RULE, Regions, SIDEBAR_WIDTH};
use super::{layout, window};

fn rail_window(width: f32) -> Input {
    Input {
        sidebar_rail: true,
        ..window(width, 834.0)
    }
}

#[test]
fn user_rail_takes_72_at_any_width() {
    for width in [605.0, 683.0, 1210.0, 1376.0] {
        let out = layout(rail_window(width));
        assert!(out.sidebar_collapsed, "width {width}");
        assert_eq!(out.sidebar.width, RAIL_WIDTH, "width {width}");
        assert_eq!(out.tab_row.x, RAIL_WIDTH + RULE, "width {width}");
        assert_eq!(
            out.tab_row.width,
            width - RAIL_WIDTH - RULE,
            "width {width}"
        );
        let full = layout(window(width, 834.0));
        assert_eq!(full.sidebar_collapsed, width < SIDEBAR_WIDTH + RULE + 280.0);
    }
}

#[test]
fn user_rail_keeps_m2_when_it_fits() {
    let regions = Regions {
        m2: ModSlot::Wants(320.0),
        ..Regions::default()
    };
    let wide = layout(Input {
        regions,
        ..rail_window(1000.0)
    });
    assert_eq!(wide.region_size(super::super::Region::M2).width, 320.0);
    assert!(wide.withdrawn.is_empty());
    // 480: 72 + 1 + 280 = 353 leaves 127, less than M2's 280 minimum.
    let narrow = layout(Input {
        regions,
        ..rail_window(480.0)
    });
    assert_eq!(narrow.withdrawn, [super::super::Region::M2]);
    assert!(narrow.terminal.rect().is_some());
}
