//! Mods shows only while a mod region is withdrawn (Q3).
use super::super::*;
use crate::layout::{self, Insets, ModSlot, Regions, Size};

fn withdrawn(window: Size, keyboard: f32, regions: Regions) -> Vec<Region> {
    layout::compute(&layout::Input {
        window,
        insets: Insets {
            keyboard,
            ..Insets::default()
        },
        regions,
        sidebar_list_height: 564.0,
        pane_count: 1,
        ..layout::Input::default()
    })
    .withdrawn
}

#[test]
fn mods_item_only_when_withdrawn() {
    assert_eq!(mods_item(0), None);
    assert_eq!(mods_item(1).as_deref(), Some("Mods 1"));
    assert_eq!(mods_item(2).as_deref(), Some("Mods 2"));
    // No mod requested (today's real screen): nothing withdrawn, no Mods.
    let none = withdrawn(Size::new(1376.0, 1032.0), 0.0, Regions::default());
    assert_eq!(mods_item(none.len()), None);
    assert_eq!(mods_accessibility_label(&none), None);
    // Artboard 11 (605×834, defaults): M2 is withdrawn, "Mods 1".
    let half = withdrawn(Size::new(605.0, 834.0), 0.0, Regions::defaults());
    assert_eq!(half, vec![Region::M2]);
    assert_eq!(mods_item(half.len()).as_deref(), Some("Mods 1"));
    assert_eq!(
        mods_accessibility_label(&half).as_deref(),
        Some("Mods, 1 withdrawn: right dock")
    );
    // Artboard 10 (1210×834, keyboard 320, maximum): M1 and M4, "Mods 2".
    let max = withdrawn(Size::new(1210.0, 834.0), 320.0, Regions::maximum());
    assert_eq!(max, vec![Region::M1, Region::M4]);
    assert_eq!(mods_item(max.len()).as_deref(), Some("Mods 2"));
    assert_eq!(
        mods_accessibility_label(&max).as_deref(),
        Some("Mods, 2 withdrawn: sidebar bottom, top band")
    );
    assert_eq!(
        mods_overlay_rows(&max),
        vec!["M1 · sidebar bottom".to_owned(), "M4 · top band".to_owned()]
    );
    // A mod with a mismatched version is absent, not withdrawn (q1).
    let mismatch = Regions {
        m2: ModSlot::VersionMismatch,
        ..Regions::defaults()
    };
    let mismatched = withdrawn(Size::new(605.0, 834.0), 0.0, mismatch);
    assert!(!mismatched.contains(&Region::M2));
    assert_eq!(mods_item(mismatched.len()), None);
}
