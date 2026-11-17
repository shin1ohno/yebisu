use super::super::{PaneMode, Region, Size};
use super::{artboard_10, artboard_11, layout, terminal_size};

#[test]
fn artboard_10_fixture() {
    let out = layout(artboard_10());
    assert_eq!(terminal_size(&out), Size::new(429.0, 206.0));
    assert_eq!(out.region_size(Region::M2), Size::new(480.0, 432.0));
    assert_eq!(out.region_size(Region::M3), Size::new(429.0, 226.0));
    assert_eq!(out.m4, None);
    assert_eq!(out.m1, None);
    assert_eq!(out.withdrawn, vec![Region::M1, Region::M4]);
    assert_eq!(out.pane_mode, PaneMode::Single);
    assert_eq!(out.sidebar.width, 300.0);
    assert!(!out.sidebar_collapsed);
    assert_eq!(out.tab_row.height, 36.0);
    assert_eq!(out.find_bar.height, 0.0);
    // M5: three sections of 160 fit the 909-wide status bar.
    assert_eq!(out.m5.shown, 3);
    assert_eq!(out.m5.width, 480.0);
}

#[test]
fn artboard_11_fixture() {
    let out = layout(artboard_11());
    assert_eq!(terminal_size(&out), Size::new(304.0, 596.0));
    assert_eq!(out.region_size(Region::M1), Size::new(300.0, 129.0));
    assert_eq!(out.m2, None);
    assert_eq!(out.region_size(Region::M3), Size::new(304.0, 112.0));
    assert_eq!(out.region_size(Region::M4), Size::new(304.0, 44.0));
    assert_eq!(out.withdrawn, vec![Region::M2]);
    assert_eq!(out.pane_mode, PaneMode::Single);
    assert_eq!(out.sidebar.width, 300.0);
    assert!(!out.sidebar_collapsed);
}
