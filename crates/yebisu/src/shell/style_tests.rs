//! Each shared value against the canvas text it comes from. The quoted
//! style is copied from the artboard line named next to it (files under
//! `yebisu-design-reference/project/`), so a changed constant fails here with
//! its source in the message.
use super::*;

/// The canvas's `#rrggbb`.
fn hex(text: &str) -> u32 {
    let digits = text
        .strip_prefix('#')
        .unwrap_or_else(|| panic!("{text} is not #rrggbb"));
    u32::from_str_radix(digits, 16).unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// The number before `px` in a quoted style such as `font-size: 13px`.
fn length(style: &str) -> f32 {
    let value = style
        .rsplit(": ")
        .next()
        .and_then(|value| value.strip_suffix("px"))
        .unwrap_or_else(|| panic!("{style} has no px length"));
    value.parse().unwrap_or_else(|e| panic!("{style}: {e}"))
}

/// `rgba(r,g,b,a)` as `0xRRGGBB` and alpha.
fn rgba(text: &str) -> (u32, f32) {
    let inner = text
        .strip_prefix("rgba(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{text} is not rgba(...)"));
    let parts = inner.split(',').collect::<Vec<_>>();
    let [r, g, b, a] = parts[..] else {
        panic!("{text} needs four parts");
    };
    let byte = |part: &str| {
        part.parse::<u32>()
            .unwrap_or_else(|e| panic!("{text}: {e}"))
    };
    let alpha = a.parse().unwrap_or_else(|e| panic!("{text}: {e}"));
    ((byte(r) << 16) | (byte(g) << 8) | byte(b), alpha)
}

#[test]
fn colors_equal_the_canvas() {
    let cases = [
        (color::BACKGROUND, "#101419", "Main.dc.html:22 background"),
        (
            color::SURFACE,
            "#1c1c22",
            "Main.dc.html:24 sidebar background",
        ),
        (color::RAISED, "#2b2933", "Main.dc.html:24 border-right"),
        (
            color::SEPARATOR,
            "#3b3945",
            "Herdr2.dc.html:127 menu separator",
        ),
        (color::MUTED, "#acb2bb", "Main.dc.html:36 HOSTS heading"),
        (color::TEXT, "#d8dee9", "Main.dc.html:22 color"),
        (color::ACCENT, "#88c0d0", "Main.dc.html:14 a{color}"),
        (color::GREEN, "#a3be8c", "SidebarSpec.dc.html:38 Done"),
        (color::YELLOW, "#ebcb8b", "SidebarSpec.dc.html:38 Working"),
        (color::RED, "#bf616a", "SidebarSpec.dc.html:38 Needs you"),
        (color::SLATE, "#4c566a", "SidebarSpec.dc.html:38 no agent"),
        (color::DISABLED, "#6c7480", "Herdr2.dc.html:129 Paste image"),
    ];
    for (value, canvas, line) in cases {
        assert_eq!(value, hex(canvas), "{line} {canvas}");
    }
    // The plan's palette (Wave 0a), each colour declared once.
    let mut palette = [
        "#101419", "#1c1c22", "#2b2933", "#3b3945", "#acb2bb", "#d8dee9", "#88c0d0", "#a3be8c",
        "#ebcb8b", "#bf616a", "#4c566a", "#6c7480",
    ]
    .map(hex)
    .to_vec();
    palette.sort_unstable();
    let mut declared = cases.iter().map(|(value, _, _)| *value).collect::<Vec<_>>();
    declared.sort_unstable();
    declared.dedup();
    assert_eq!(declared, palette);
}

#[test]
fn tints_equal_the_canvas() {
    let cases = [
        (tint::SELECTED, "rgba(136,192,208,0.22)", "Main.dc.html:132"),
        (
            tint::CURRENT_ROW,
            "rgba(136,192,208,0.15)",
            "Main.dc.html:49",
        ),
        (
            tint::SELECTION,
            "rgba(136,192,208,0.38)",
            "Selection.dc.html:79",
        ),
        (
            tint::SHEET_SCRIM,
            "rgba(16,20,25,0.66)",
            "Settings.dc.html:22",
        ),
        (
            tint::POPOVER_SCRIM,
            "rgba(16,20,25,0.45)",
            "Shortcuts.dc.html:20",
        ),
    ];
    for (value, canvas, line) in cases {
        let (rgb, alpha) = rgba(canvas);
        assert_eq!((value.rgb, value.alpha), (rgb, alpha), "{line} {canvas}");
    }
}

#[test]
fn tint_converts_to_the_gpui_colour() {
    let color = Rgba::from(tint::SELECTION);
    assert_eq!(
        (color.r, color.g, color.b, color.a),
        (136. / 255., 192. / 255., 208. / 255., 0.38)
    );
}

#[test]
fn shadows_equal_the_canvas() {
    let cases = [
        (shadow::SHEET, "0 20px 60px", 0.55, "Settings.dc.html:24"),
        (shadow::POPOVER, "0 12px 36px", 0.5, "Herdr2.dc.html:123"),
        (shadow::EDIT_BAR, "0 6px 18px", 0.45, "Selection.dc.html:84"),
    ];
    for (value, offsets, alpha, line) in cases {
        let parts = offsets
            .split(' ')
            .map(|part| part.trim_end_matches("px").parse::<f32>())
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(
            vec![0., value.y, value.blur],
            parts,
            "{line} box-shadow: {offsets}"
        );
        assert_eq!(value.alpha, alpha, "{line} rgba(0,0,0,{alpha})");
    }
    let drawn = shadow::SHEET.box_shadow();
    assert_eq!(drawn.offset, point(px(0.), px(20.)));
    assert_eq!(drawn.blur_radius, px(60.));
    assert_eq!(drawn.spread_radius, px(0.));
    assert_eq!(drawn.color.a, 0.55);
    assert!(!drawn.inset);
}

#[test]
fn radii_equal_the_canvas() {
    let cases = [
        (radius::SHEET, "border-radius: 18px", "Settings.dc.html:24"),
        (radius::POPOVER, "border-radius: 14px", "Herdr2.dc.html:123"),
        (radius::CARD, "border-radius: 12px", "AddHost.dc.html:64"),
        (
            radius::EDIT_BAR,
            "border-radius: 10px",
            "Selection.dc.html:84",
        ),
        (radius::CONTROL, "border-radius: 8px", "Main.dc.html:26"),
        (radius::RAIL_ITEM, "border-radius: 6px", "Rail.dc.html:26"),
        (radius::BADGE, "border-radius: 4px", "Main.dc.html:63"),
    ];
    for (value, canvas, line) in cases {
        assert_eq!(value, length(canvas), "{line} {canvas}");
    }
}

#[test]
fn text_sizes_and_weights_equal_the_canvas() {
    let sizes = [
        (text::BASE, "font-size: 13px", "Main.dc.html:22"),
        (text::NAME, "font-size: 14px", "Main.dc.html:41"),
        (text::SECONDARY, "font-size: 12px", "Main.dc.html:42"),
        (text::SMALL, "font-size: 11px", "Main.dc.html:36"),
        (text::SMALL, "font-size: 11px", "Rail.dc.html:28"),
        (text::SHEET_TITLE, "font-size: 15px", "Settings.dc.html:27"),
        (text::PAGE_TITLE, "font-size: 18px", "Settings.dc.html:48"),
        (
            text::PAGE_TITLE_LINE,
            "line-height: 26px",
            "Settings.dc.html:48",
        ),
        (text::AGENT_LINE, "line-height: 16px", "Main.dc.html:93"),
    ];
    for (value, canvas, line) in sizes {
        assert_eq!(value, length(canvas), "{line} {canvas}");
    }
    // Main.dc.html:36 `letter-spacing: 0.06em`.
    assert_eq!(text::HEADING_TRACKING_EM, 0.06);
    let weights = [
        (weight::MEDIUM, 500., "Main.dc.html:89"),
        (weight::SEMIBOLD, 600., "Main.dc.html:36"),
        (weight::BOLD, 700., "Main.dc.html:41"),
        (REGULAR, 400., "unset"),
    ];
    for (value, canvas, line) in weights {
        assert_eq!(value, FontWeight(canvas), "{line} font-weight: {canvas}");
    }
}

#[test]
fn metrics_equal_the_canvas() {
    let cases = [
        (metric::TOUCH, "width: 44px", "Main.dc.html:26"),
        (metric::DOT, "width: 10px", "Main.dc.html:39"),
        (metric::SMALL_DOT, "width: 8px", "Main.dc.html:134"),
        (
            metric::RING,
            "border: 2px solid #ebcb8b",
            "SidebarMixed.dc.html:63",
        ),
        (metric::HEADING_ROW, "height: 32px", "Main.dc.html:36"),
        (metric::HEADING_GAP, "margin-top: 12px", "Main.dc.html:88"),
        (metric::SIDE, "padding: 0 16px", "Main.dc.html:36"),
        (metric::ROW_GAP, "gap: 10px", "Main.dc.html:38"),
        (metric::INLINE_GAP, "gap: 8px", "Main.dc.html:160"),
        (
            metric::SPACE_INDENT,
            "padding: 0 12px 0 40px",
            "Main.dc.html:58",
        ),
        (
            metric::CURRENT_LINE,
            "border-left: 2px solid {{accent}}",
            "Main.dc.html:49",
        ),
        (
            metric::DETAIL_INDENT,
            "padding-left: 18px",
            "Main.dc.html:55",
        ),
        (
            metric::LINE,
            "border: 1px solid {{accent}}",
            "Main.dc.html:161",
        ),
        (metric::PANE_NAME_INSET, "left: 10px", "Main.dc.html:161"),
        (metric::MENU_WIDTH, "width: 260px", "Herdr2.dc.html:123"),
        (metric::MENU_ITEM, "height: 48px", "Herdr2.dc.html:129"),
        (metric::MENU_PADDING, "padding: 6px 0", "Herdr2.dc.html:123"),
        (metric::SHEET_HEADER, "height: 60px", "Settings.dc.html:25"),
        (metric::SETTINGS_LIST, "width: 240px", "Settings.dc.html:32"),
        (metric::SETTINGS_ROW, "height: 56px", "Settings.dc.html:34"),
        (metric::SEGMENT, "height: 40px", "Settings.dc.html:57"),
        (metric::SEGMENT_INSET, "padding: 2px", "Settings.dc.html:56"),
    ];
    for (value, canvas, line) in cases {
        // The last length of the quoted style is the one the constant takes.
        let last = canvas
            .split(' ')
            .rfind(|part| part.ends_with("px"))
            .unwrap_or_else(|| panic!("{canvas} has no px length"));
        assert_eq!(value, length(&format!("x: {last}")), "{line} {canvas}");
    }
    // A current space row loses as much inset as its accent line is wide
    // (Main.dc.html:49 `padding: 0 12px 0 38px; border-left: 2px`).
    assert_eq!(metric::SPACE_INDENT - metric::CURRENT_LINE, 38.);
}

/// The root draws unsized text at `text::BASE`, not GPUI's 16 (app.rs). A
/// tab's label sets no size, so its one line is BASE times GPUI's line
/// height (phi) tall.
#[gpui::test]
fn root_draws_unsized_text_at_the_canvas_base(cx: &mut gpui::TestAppContext) {
    use crate::{app::Smoke, mock_peer};
    use std::time::{Duration, Instant};
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(gpui::size(px(1376.), px(1032.)));
    let (client, _receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("root-text");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    let label = cx
        .debug_bounds("shell-tab-body-0")
        .unwrap_or_else(|| panic!("the first tab label was not painted"));
    // GPUI's default line height is `phi()`, the golden ratio.
    let phi = std::f32::consts::GOLDEN_RATIO;
    let height = f32::from(label.size.height);
    assert!(
        (height - text::BASE * phi).abs() < 0.5,
        "tab label line {height} pt, expected {} for {} pt text (GPUI's 16 pt gives {})",
        text::BASE * phi,
        text::BASE,
        16. * phi
    );
}
