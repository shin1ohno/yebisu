use crate::{
    layout::{Input, ModSlot, Regions, Size, compute},
    shell::rail::{
        FULL_MIN_WINDOW, RAIL_FOOTER_HEIGHT, RAIL_HEADER_HEIGHT, SidebarMode, rail_sections,
        sidebar_mode,
    },
};

fn collapsed_by_layout(width: f32, regions: Regions) -> bool {
    compute(&Input {
        window: Size::new(width, 834.0),
        regions,
        sidebar_list_height: 564.0,
        pane_count: 1,
        ..Input::default()
    })
    .sidebar_collapsed
}

#[test]
fn sidebar_collapses_last() {
    // Same answer as L0's allocation at every width, with and without M2:
    // M2 gives way first, so it never moves the collapse point.
    let with_m2 = Regions {
        m2: ModSlot::Wants(320.0),
        ..Regions::default()
    };
    for width in 300..=1400 {
        let width = width as f32;
        let mode = sidebar_mode(width, SidebarMode::Full);
        for regions in [Regions::default(), with_m2] {
            assert_eq!(
                mode == SidebarMode::Rail,
                collapsed_by_layout(width, regions),
                "width {width}"
            );
        }
    }
    // The cases the plan names.
    assert_eq!(FULL_MIN_WINDOW, 581.0);
    assert_eq!(sidebar_mode(480.0, SidebarMode::Full), SidebarMode::Rail);
    assert_eq!(sidebar_mode(580.9, SidebarMode::Full), SidebarMode::Rail);
    assert_eq!(sidebar_mode(581.0, SidebarMode::Full), SidebarMode::Full);
    // Split View half (683 pt): 300 plus one 280 pane fits.
    assert_eq!(sidebar_mode(683.0, SidebarMode::Full), SidebarMode::Full);
    assert_eq!(sidebar_mode(605.0, SidebarMode::Full), SidebarMode::Full);
    // The user's choice wins at any width, and a wide window does not undo it.
    assert_eq!(sidebar_mode(1376.0, SidebarMode::Rail), SidebarMode::Rail);
    assert_eq!(sidebar_mode(1376.0, SidebarMode::Full), SidebarMode::Full);
}

#[test]
fn header_and_footer_stay_fixed() {
    let available = 834.0;
    let reference = rail_sections(available, 0.0);
    assert_eq!(
        reference.header,
        RAIL_HEADER_HEIGHT + 44.0 + 6.0 + 1.0 + 6.0
    );
    assert_eq!(reference.footer, RAIL_FOOTER_HEIGHT);
    for content in [0.0, 56.0, 400.0, 5000.0] {
        let sections = rail_sections(available, content);
        assert_eq!(sections.header, reference.header, "content {content}");
        assert_eq!(sections.footer, reference.footer, "content {content}");
        assert_eq!(
            sections.header + sections.list + sections.footer,
            available,
            "the list takes exactly what the fixed parts leave"
        );
        assert_eq!(sections.list_scrolls, content > sections.list);
    }
    assert!(!rail_sections(available, 100.0).list_scrolls);
    assert!(rail_sections(available, 5000.0).list_scrolls);
    // A window shorter than the fixed parts leaves the list nothing, not less.
    let short = rail_sections(100.0, 56.0);
    assert_eq!(short.list, 0.0);
    assert_eq!(
        (short.header, short.footer),
        (reference.header, RAIL_FOOTER_HEIGHT)
    );
}
