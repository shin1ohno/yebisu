use super::super::{Input, Layout, M2_RANGE, M3_RANGE, M4_RANGE, ModSlot, Region, Regions};
use super::{layout, window};

fn wide(regions: Regions) -> Layout {
    // Room for every region at its maximum: 1376 × 1032, no keyboard.
    layout(Input {
        regions,
        ..window(1376.0, 1032.0)
    })
}

#[test]
fn m2_clamped_to_280_480() {
    assert_eq!(
        (M2_RANGE.min, M2_RANGE.default, M2_RANGE.max),
        (280.0, 320.0, 480.0)
    );
    for (asked, given) in [
        (100.0, 280.0),
        (280.0, 280.0),
        (320.0, 320.0),
        (480.0, 480.0),
        (900.0, 480.0),
    ] {
        let out = wide(Regions {
            m2: ModSlot::Wants(asked),
            ..Regions::default()
        });
        assert_eq!(out.region_size(Region::M2).width, given, "asked {asked}");
    }
    assert_eq!(Regions::defaults().m2, ModSlot::Wants(320.0));
}

#[test]
fn m3_clamped_to_44_320() {
    assert_eq!(
        (M3_RANGE.min, M3_RANGE.default, M3_RANGE.max),
        (44.0, 112.0, 320.0)
    );
    for (asked, given) in [
        (0.5, 44.0),
        (44.0, 44.0),
        (112.0, 112.0),
        (320.0, 320.0),
        (700.0, 320.0),
    ] {
        let out = wide(Regions {
            m3: ModSlot::Wants(asked),
            ..Regions::default()
        });
        assert_eq!(out.region_size(Region::M3).height, given, "asked {asked}");
    }
    assert_eq!(Regions::defaults().m3, ModSlot::Wants(112.0));
}

#[test]
fn m4_clamped_to_28_160() {
    assert_eq!(
        (M4_RANGE.min, M4_RANGE.default, M4_RANGE.max),
        (28.0, 44.0, 160.0)
    );
    for (asked, given) in [
        (1.0, 28.0),
        (28.0, 28.0),
        (44.0, 44.0),
        (160.0, 160.0),
        (400.0, 160.0),
    ] {
        let out = wide(Regions {
            m4: ModSlot::Wants(asked),
            ..Regions::default()
        });
        assert_eq!(out.region_size(Region::M4).height, given, "asked {asked}");
    }
    assert_eq!(Regions::defaults().m4, ModSlot::Wants(44.0));
}
