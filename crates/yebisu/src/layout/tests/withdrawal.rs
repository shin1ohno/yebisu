use super::super::{
    Input, M1_MIN_HEIGHT, ModPresence, ModSlot, RAIL_WIDTH, Region, Regions, SIDEBAR_WIDTH,
};
use super::{LIST_HEIGHT, artboard_10, artboard_11, layout, terminal_size, window};

#[test]
fn width_shortage_withdraws_dock_before_sidebar() {
    let with_dock = |width| {
        layout(Input {
            regions: Regions {
                m2: ModSlot::Wants(320.0),
                ..Regions::default()
            },
            ..window(width, 834.0)
        })
    };
    // 300 + 1 + 280 + 320: everything fits.
    let full = with_dock(901.0);
    assert_eq!(
        (full.sidebar.width, full.region_size(Region::M2).width),
        (300.0, 320.0)
    );
    // One point less: M2 gives up width first, the sidebar stays.
    let shrunk = with_dock(900.0);
    assert_eq!(
        (shrunk.sidebar.width, shrunk.region_size(Region::M2).width),
        (300.0, 319.0)
    );
    assert_eq!(terminal_size(&shrunk).width, 280.0);
    // Below M2's minimum: M2 is withdrawn and the sidebar still stays.
    let withdrawn = with_dock(860.0);
    assert_eq!(withdrawn.m2, None);
    assert_eq!(withdrawn.sidebar.width, SIDEBAR_WIDTH);
    assert_eq!(withdrawn.withdrawn, vec![Region::M2]);
    assert_eq!(terminal_size(&withdrawn).width, 559.0);
    // Only once M2 is gone does the sidebar collapse to the rail.
    let rail = with_dock(580.0);
    assert!(rail.sidebar_collapsed);
    assert_eq!(rail.sidebar.width, RAIL_WIDTH);
    assert_eq!(rail.m2, None);
    assert_eq!(terminal_size(&rail).width, 580.0 - 73.0);
    let last = with_dock(581.0);
    assert!(!last.sidebar_collapsed);
    assert_eq!(terminal_size(&last).width, 280.0);
}

#[test]
fn height_shortage_withdraws_m4_then_shrinks_m3() {
    let tall = |m3, m4, height| {
        layout(Input {
            regions: Regions {
                m3: ModSlot::Wants(m3),
                m4: ModSlot::Wants(m4),
                ..Regions::default()
            },
            ..window(1000.0, height)
        })
    };
    // Column 82 less than the window. 44 + 112 + 206 = 362 fits exactly.
    let fits = tall(112.0, 44.0, 82.0 + 362.0);
    assert_eq!(fits.region_size(Region::M4).height, 44.0);
    assert_eq!(fits.region_size(Region::M3).height, 112.0);
    // One point short: M4 goes whole, M3 keeps its full 112.
    let m4_out = tall(112.0, 44.0, 82.0 + 361.0);
    assert_eq!(m4_out.m4, None);
    assert_eq!(m4_out.region_size(Region::M3).height, 112.0);
    assert_eq!(terminal_size(&m4_out).height, 249.0);
    // The intermediate state: with M4 gone M3 still cannot have its maximum
    // and takes the rest (column 432: 432 − 206 = 226 of 320).
    let between = tall(320.0, 160.0, 82.0 + 432.0);
    assert_eq!(between.m4, None);
    assert_eq!(between.region_size(Region::M3).height, 226.0);
    assert_eq!(terminal_size(&between).height, 206.0);
    assert_eq!(between.withdrawn, vec![Region::M4]);
    // Below 206 + 44: M3 goes too.
    let both = tall(320.0, 160.0, 82.0 + 249.0);
    assert_eq!((both.m3, both.m4), (None, None));
    assert_eq!(both.withdrawn, vec![Region::M3, Region::M4]);
    assert_eq!(terminal_size(&both).height, 249.0);
}

#[test]
fn sidebar_bottom_needs_120() {
    assert_eq!(M1_MIN_HEIGHT, 120.0);
    // Sidebar = window height: header 44 + list + M1 + footer 97.
    let with_m1 = |height| {
        layout(Input {
            regions: Regions {
                m1: ModPresence::Present,
                ..Regions::default()
            },
            ..window(1000.0, height)
        })
    };
    let fits = with_m1(44.0 + LIST_HEIGHT + 120.0 + 97.0);
    let m1 = fits.m1.unwrap_or_else(|| panic!("M1 shown"));
    assert_eq!((m1.width, m1.height), (300.0, 120.0));
    assert_eq!(m1.y, 44.0 + LIST_HEIGHT);
    let short = with_m1(44.0 + LIST_HEIGHT + 119.0 + 97.0);
    assert_eq!(short.m1, None);
    assert_eq!(short.withdrawn, vec![Region::M1]);
    // The rail has no list and no M1.
    let rail = layout(Input {
        regions: Regions {
            m1: ModPresence::Present,
            ..Regions::default()
        },
        ..window(500.0, 1032.0)
    });
    assert!(rail.sidebar_collapsed);
    assert_eq!(rail.m1, None);
}

#[test]
fn withdrawn_regions_are_listed() {
    assert_eq!(
        layout(artboard_10()).withdrawn,
        vec![Region::M1, Region::M4]
    );
    assert_eq!(layout(artboard_11()).withdrawn, vec![Region::M2]);
    // Shown and empty regions are not listed.
    assert!(layout(window(1376.0, 1032.0)).withdrawn.is_empty());
    let shown = layout(Input {
        regions: Regions::defaults(),
        ..window(1376.0, 1032.0)
    });
    assert!(shown.withdrawn.is_empty(), "{:?}", shown.withdrawn);
}
