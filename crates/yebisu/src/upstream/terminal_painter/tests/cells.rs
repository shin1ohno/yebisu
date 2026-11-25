// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/terminal_painter/tests/cells.rs at 398c132a06723a0cc1494842a34666d173ec0200
use super::*;

#[test]
fn link_underlines_stay_inside_the_frame() {
    let frame = FrameData {
        width: 10,
        height: 3,
        cells: vec![cell("a"); 30],
        cursor: None,
        hyperlinks: vec![],
        graphics: vec![],
    };
    let rect = |x, y, width, height| SurfaceRect {
        x,
        y,
        width,
        height,
    };
    // A wrapped link's two rows, the second past the frame's right edge.
    assert_eq!(
        link_bounds(&frame, &[(1, 4..10), (2, 0..14)]),
        Some(rect(0, 1, 10, 2))
    );
    // Rows a resize has already removed, and empty ranges, paint nothing.
    assert_eq!(link_bounds(&frame, &[(3, 0..4), (0, 5..5)]), None);
    assert_eq!(link_bounds(&frame, &[(0, 12..14)]), None);
    assert_eq!(link_bounds(&frame, &[]), None);
}

#[gpui::test]
fn edge_backgrounds_reach_the_canvas_without_stretching_popups_or_prompts(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    for (extend, prompt) in [(false, false), (true, false), (false, true), (true, true)] {
        cx.draw(Point::default(), size(px(100.), px(100.)), |_, _| {
            canvas(
                |_, _, _| (),
                move |_, _, window, cx| {
                    let frame = FrameData {
                        width: 2,
                        height: 2,
                        cells: [0x123456, 0x654321, 0xabcdef, 0xfedcba]
                            .into_iter()
                            .enumerate()
                            .map(|(index, bg)| CellData {
                                bg: 0x02000000 | bg,
                                ..cell(if prompt && index == 3 {
                                    "\u{e0b0}"
                                } else {
                                    " "
                                })
                            })
                            .collect(),
                        cursor: None,
                        hyperlinks: vec![],
                        graphics: vec![],
                    };
                    let mut painter = TerminalPainter::default();
                    painter.set_appearance(14., 20., Theme::default());
                    painter.paint_frame(
                        &frame,
                        point(px(17.), px(23.)),
                        extend.then(|| size(px(23.), px(47.))),
                        Ground::Opaque,
                        10.,
                        &font("Menlo"),
                        &[],
                        &[],
                        None,
                        None,
                        window,
                        cx,
                    );
                },
            )
            .size_full()
        });
        cx.update(|window, _| {
            let quads = window.painted_quads();
            let right = if extend { 13. } else { 10. };
            // A separator cap stays cell-high, so its row does not continue below.
            let bottom = if extend && !prompt { 27. } else { 20. };
            let expected = [
                (17., 23., 10., 20., 0x123456),
                (27., 23., right, 20., 0x654321),
                (17., 43., 10., bottom, 0xabcdef),
                (27., 43., right, bottom, 0xfedcba),
            ];
            assert_eq!(quads.len(), expected.len());
            for (x, y, width, height, color) in expected {
                let bounds = Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
                    .scale(window.scale_factor());
                assert!(
                    quads
                        .iter()
                        .any(|quad| quad.bounds == bounds && quad.background == rgb(color).into()),
                    "extend={extend} prompt={prompt} {bounds:?}"
                );
            }
        });
    }
}

#[test]
fn backgrounds_fill_only_fractional_cell_remainders() {
    let cell = size(px(10.), px(20.));
    let available = size(px(103.), px(67.));
    let viewport = viewport(103., 67., 10., 20.);
    let grid = size(
        px(f32::from(viewport.cols) * 10.),
        px(f32::from(viewport.rows) * 20.),
    );
    assert_eq!(grid, size(px(100.), px(60.)));
    assert_eq!(background_extent(grid, available, cell), available);
    for (available, expected) in [
        (size(px(100.), px(60.)), grid),
        (size(px(99.), px(59.)), grid),
        (size(px(110.), px(80.)), grid),
        (size(px(111.), px(67.)), size(px(100.), px(67.))),
        (size(px(103.), px(81.)), size(px(103.), px(60.))),
    ] {
        assert_eq!(background_extent(grid, available, cell), expected);
    }
}

#[test]
fn decorations_cover_spaces_empty_and_wide_continuation_cells() {
    for (symbol, skip) in [("x", false), (" ", false), ("", false), ("", true)] {
        let mut cell = CellData {
            skip,
            ..cell(symbol)
        };
        assert_eq!(decoration_offsets(&cell, CELL_HEIGHT).count(), 0);
        cell.modifier = UNDERLINE;
        assert_eq!(
            decoration_offsets(&cell, CELL_HEIGHT).collect::<Vec<_>>(),
            vec![18.]
        );
        cell.modifier = STRIKETHROUGH;
        assert_eq!(
            decoration_offsets(&cell, CELL_HEIGHT).collect::<Vec<_>>(),
            vec![10.]
        );
        cell.modifier = UNDERLINE | STRIKETHROUGH;
        assert_eq!(
            decoration_offsets(&cell, CELL_HEIGHT).collect::<Vec<_>>(),
            vec![18., 10.]
        );
        assert_eq!(
            decoration_offsets(&cell, 30.5).collect::<Vec<_>>(),
            vec![28.5, 15.25]
        );
    }
}

#[test]
fn spans_cover_skip_cells_and_resolved_colors_without_crossing_rows() {
    let theme = Theme::default();
    let mut row = vec![cell("\u{754c}"), cell(""), cell("x"), cell("x")];
    row[1].skip = true;
    row[2].fg = 0x02123456;
    row[2].modifier = 64;
    row[3].bg = 0x02123456;
    assert_eq!(
        background_spans(&row, 0..row.len(), &theme).collect::<Vec<_>>(),
        vec![(0, 2, BACKGROUND, true), (2, 4, 0x123456, false)]
    );
    assert_eq!(background_spans(&[], 0..0, &theme).count(), 0);
    for cells in row.chunks(2) {
        let expanded: Vec<_> = background_spans(cells, 0..cells.len(), &theme)
            .flat_map(|(a, b, color, _)| (a..b).map(move |_| color))
            .collect();
        assert_eq!(
            expanded,
            cells
                .iter()
                .map(|c| cell_colors(c, &theme).1)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn wide_continuation_cells_take_the_glyph_background() {
    let theme = Theme::default();
    // The daemon sends continuation cells with a background of their own.
    let mut row = vec![cell("\u{3053}"), cell(""), cell("x"), cell("")];
    row[0].bg = 0x02373737;
    row[1].bg = 0x02000000;
    row[3].bg = 0x02000000;
    assert_eq!(
        background_spans(&row, 0..row.len(), &theme).collect::<Vec<_>>(),
        vec![
            (0, 2, 0x373737, false),
            (2, 3, BACKGROUND, true),
            (3, 4, 0, false),
        ]
    );
    // Halfwidth katakana with a voiced or semi-voiced mark is two columns
    // wide in Herdr although its Unicode width is one.
    for kana in ["\u{ff76}\u{ff9e}", "\u{ff8a}\u{ff9f}"] {
        let mut row = vec![cell(kana), cell(""), cell("\u{ff76}"), cell("")];
        row[0].bg = 0x02373737;
        row[1].bg = 0x02000000;
        row[3].bg = 0x02000000;
        assert_eq!(
            background_spans(&row, 0..row.len(), &theme).collect::<Vec<_>>(),
            vec![
                (0, 2, 0x373737, false),
                (2, 3, BACKGROUND, true),
                (3, 4, 0, false),
            ],
            "{kana}"
        );
    }
}

#[test]
fn spans_and_styles_use_custom_theme() {
    let mut theme = Theme {
        background: 0x123456,
        foreground: 0xabcdef,
        ..Theme::default()
    };
    theme.palette[200] = theme.background;
    theme.palette[1] = 0x654321;
    let row = [
        cell("x"),
        CellData {
            bg: 0x010000c8,
            fg: 2,
            ..cell("y")
        },
    ];
    assert_eq!(
        background_spans(&row, 0..row.len(), &theme).collect::<Vec<_>>(),
        // The same color, but only the first is the theme's own: a backdrop
        // shows through it and not through the color the program chose.
        vec![
            (0, 1, theme.background, true),
            (1, 2, theme.background, false)
        ]
    );
    assert_eq!(cell_colors(&row[0], &theme).0, theme.foreground);
    assert_eq!(cell_colors(&row[1], &theme).0, theme.palette[1]);
}

/// Issue #137 PR 6 (yebisu): a pane with a scroll position but no bar column
/// (Herdr 2) paints its thumb over its last column, whose cells still paint.
#[gpui::test]
fn a_pane_without_a_bar_column_paints_its_thumb_over_its_last_column(cx: &mut TestAppContext) {
    use sheprd_client::protocol::{PaneSurfacePane, PaneSurfaceScrollMetrics};
    const LAST: u32 = 0x00aa00;
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    cx.draw(Point::default(), size(px(200.), px(200.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |_, _, window, cx| {
                let frame = FrameData {
                    width: 4,
                    height: 4,
                    cells: (0..16u32)
                        .map(|index| CellData {
                            bg: 0x02000000
                                | if index % 4 == 3 {
                                    LAST
                                } else {
                                    0x101010 + index
                                },
                            ..cell(" ")
                        })
                        .collect(),
                    cursor: None,
                    hyperlinks: vec![],
                    graphics: vec![],
                };
                let rect = SurfaceRect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 4,
                };
                let pane = PaneSurfacePane {
                    pane_id: "pane".into(),
                    content_revision: 1,
                    rect,
                    inner_rect: rect,
                    scrollbar_rect: None,
                    scroll: Some(PaneSurfaceScrollMetrics {
                        offset_from_bottom: 10,
                        max_offset_from_bottom: 20,
                        viewport_rows: 4,
                    }),
                    focused: true,
                    mouse_reporting: false,
                    sgr_pixel_mouse: false,
                    alternate_screen_active: false,
                    pixel_width: 0,
                    pixel_height: 0,
                };
                let mut painter = TerminalPainter::default();
                painter.set_appearance(14., 20., Theme::default());
                painter.paint_frame(
                    &frame,
                    Point::default(),
                    None,
                    Ground::Opaque,
                    10.,
                    &font("Menlo"),
                    &[],
                    std::slice::from_ref(&pane),
                    None,
                    None,
                    window,
                    cx,
                );
            },
        )
        .size_full()
    });
    cx.update(|window, _| {
        let quads = window.painted_quads();
        let scale = window.scale_factor();
        for row in 0..4 {
            let bounds = Bounds::new(point(px(30.), px(row as f32 * 20.)), size(px(10.), px(20.)))
                .scale(scale);
            assert!(
                quads
                    .iter()
                    .any(|quad| quad.bounds == bounds && quad.background == rgb(LAST).into()),
                "the last column's cell on row {row} still paints"
            );
        }
        let thumb = rgba((Theme::default().muted << 8) | 0xc0);
        let thumbs: Vec<_> = quads
            .iter()
            .filter(|quad| quad.background == thumb.into())
            .collect();
        assert_eq!(thumbs.len(), 1, "one thumb");
        let column = Bounds::new(point(px(30.), px(0.)), size(px(10.), px(80.))).scale(scale);
        let bounds = thumbs[0].bounds;
        assert!(
            bounds.left() >= column.left() && bounds.right() <= column.right(),
            "the thumb rides the last column: {bounds:?}"
        );
    });
}
