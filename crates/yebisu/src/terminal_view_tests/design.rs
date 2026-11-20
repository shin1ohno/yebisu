use crate::{app::Smoke, handoff::Selection, input_geometry::InputGeometry, mock_peer};
use gpui::{
    Bounds, ClipboardItem, Empty, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, TestAppContext,
    VisualTestContext, canvas, font, point, prelude::*, px, rgb, size,
};
use sheprd_client::protocol::{FrameData, PaneSurfaceFrame, PaneSurfacePane, SurfaceRect};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[gpui::test]
fn no_find_band_above_the_terminal_while_find_is_closed(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    assert!(view.read_with(cx, |this, _| this.find.is_none()));
    let area = cx
        .debug_bounds("shell-terminal")
        .unwrap_or_else(|| panic!("terminal slot"));
    let terminal = cx
        .debug_bounds("terminal-input")
        .unwrap_or_else(|| panic!("terminal input"));
    assert_eq!(
        terminal.top(),
        area.top(),
        "closed Find reserves terminal space"
    );
}

#[gpui::test]
fn terminal_padding_follows_the_canvas(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let geometry = view.read_with(cx, |this, _| this.terminal.geometry);
    let area = cx
        .debug_bounds("terminal-input")
        .unwrap_or_else(|| panic!("terminal input"));
    let grid = geometry.grid;
    assert_eq!(grid.origin - area.origin, point(px(14.), px(12.)));
    assert_eq!(area.size - grid.size, size(px(28.), px(24.)));
    view.read_with(cx, |this, _| {
        let surface = this
            .terminal
            .live
            .surface
            .as_deref()
            .unwrap_or_else(|| panic!("surface"));
        let cursor = surface
            .frame
            .cursor
            .as_ref()
            .unwrap_or_else(|| panic!("cursor"));
        let (width, height) = (geometry.cell_width, geometry.cell_height);
        let candidate = geometry.cursor(Some(surface));
        assert_eq!(
            candidate.origin,
            grid.origin
                + point(
                    px(f32::from(cursor.x) * width),
                    px(f32::from(cursor.y) * height)
                )
        );
        assert_eq!(
            geometry.cell_at(surface, candidate.origin, None),
            Some((cursor.x, cursor.y))
        );
        assert_eq!(geometry.cell_at(surface, area.origin, None), Some((0, 0)));
    });
}

#[gpui::test]
fn pane_borders_draw_the_name_above_a_one_point_border(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let surface = split_surface();
    let geometry = InputGeometry::from_canvas(
        Bounds::new(point(px(0.), px(0.)), size(px(800.), px(500.))),
        10.,
        20.,
    );
    let panes = geometry.framed_panes(&surface);
    let [left, right] = panes.as_slice() else {
        panic!("expected two pane outlines")
    };
    assert_eq!(right.outline.left() - left.outline.right(), px(8.));
    assert_eq!(
        left.body.origin - left.outline.origin,
        point(px(13.), px(15.))
    );
    let gap = point(
        (left.outline.right() + right.outline.left()) / 2.,
        left.body.top(),
    );
    assert_eq!(geometry.cell_at(&surface, gap, None), None);
    assert_eq!(
        geometry.cell_at(&surface, right.body.origin, None),
        Some((41, 0))
    );

    let outline = right.outline;
    let left_outline = left.outline;
    let canvas_bounds = geometry.canvas_bounds();
    cx.draw(canvas_bounds.origin, canvas_bounds.size, |_, _| {
        canvas(
            |_, _, _| (),
            move |_, _, window, cx| {
                let mut painter = crate::terminal_painter::TerminalPainter::default();
                let assets = sheprd_client::SurfaceImages::default();
                for (placement, label) in geometry
                    .framed_panes(&surface)
                    .into_iter()
                    .zip(["claude", "zsh"])
                {
                    painter.paint_framed_pane(
                        &surface,
                        placement,
                        label,
                        10.,
                        &font("Menlo"),
                        &[],
                        &assets,
                        window,
                        cx,
                    );
                }
            },
        )
        .size_full()
    });
    cx.update(|window, _| {
        let quads = window.painted_quads();
        let scale = window.scale_factor();
        let snap = |point: gpui::Point<gpui::Pixels>| {
            point.map(|value| window.pixel_snap(value).scale(scale))
        };
        for (outline, border_color) in [(outline, 0x88c0d0), (left_outline, 0x4c566a)] {
            let outline_bounds =
                Bounds::from_corners(snap(outline.origin), snap(outline.bottom_right()));
            let painted = quads
                .iter()
                .find(|quad| quad.bounds == outline_bounds)
                .unwrap_or_else(|| panic!("missing border at {outline:?}"));
            assert_eq!(painted.border_color, rgb(border_color).into());
            assert_eq!(painted.border_widths.top, px(1.).scale(scale));
            assert_eq!(painted.border_widths.right, px(1.).scale(scale));
            let name_origin = snap(outline.origin + point(px(10.), px(-10.)));
            assert!(
                quads.iter().any(|quad| {
                    quad.bounds.origin == name_origin && quad.background == rgb(0x101419).into()
                }),
                "missing name background across the top line"
            );
        }
    });
}

#[test]
fn copy_as_one_line_joins_wrapped_rows() {
    let mut surface = split_surface();
    surface
        .frame
        .cells
        .iter_mut()
        .for_each(|cell| cell.symbol = " ".into());
    let width = usize::from(surface.frame.width);
    for (index, symbol) in [
        (38, "a"),
        (39, "日"),
        (41, "X"),
        (width, "b"),
        (width + 1, "c"),
        (2 * width, "d"),
    ] {
        surface.frame.cells[index].symbol = symbol.into();
    }
    surface.frame.cells[40].symbol.clear();
    let selection = Selection {
        frame: Arc::new(surface.frame.clone()),
        anchor: (38, 0),
        end: (0, 2),
        dragging: false,
    };
    assert_eq!(
        selection.surface_text(&surface, true).as_deref(),
        Some("a日bc\nd")
    );
    let mut selection = selection;
    selection.anchor = (0, 2);
    selection.end = (38, 0);
    assert_eq!(
        selection.surface_text(&surface, true).as_deref(),
        Some("a日bc\nd")
    );
    assert!(
        selection
            .surface_text(&surface, false)
            .is_some_and(|s| s.starts_with("a日\nbc") && !s.contains('X'))
    );
    selection.anchor = (1, 1);
    selection.end = (4, 1);
    assert_eq!(
        selection.surface_text(&surface, true).as_deref(),
        Some("c   ")
    );
}

#[gpui::test]
fn option_command_c_copies_as_one_line(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    cx.write_to_clipboard(ClipboardItem::new_string("clipboard sentinel".into()));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.sync_handoff();
            assert!(this.handoff.stamp.is_some());
            let surface = this
                .terminal
                .live
                .surface
                .as_mut()
                .unwrap_or_else(|| panic!("surface"));
            let frame = &mut Arc::make_mut(surface).frame;
            frame.width = 3;
            frame.height = 2;
            frame.cells.truncate(6);
            for (cell, symbol) in frame.cells.iter_mut().zip([" ", "a", "b", "c", " ", " "]) {
                cell.symbol = symbol.into();
            }
            this.handoff.selection = Some(Selection {
                frame: Arc::new(frame.clone()),
                anchor: (1, 0),
                end: (0, 1),
                dragging: false,
            });
            let keystroke = Keystroke {
                key: "c".into(),
                key_char: None,
                modifiers: Modifiers {
                    platform: true,
                    alt: true,
                    ..Modifiers::none()
                },
            };
            for is_held in [false, true] {
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
            }
            this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
            assert_eq!(this.handoff.clipboard_writes, 1);
        });
    });
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("abc")
    );
}

fn terminal(cx: &mut TestAppContext) -> (gpui::Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    cx.update(|window, _| window.activate_window());
    let (client, _receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("terminal-design");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < deadline, "private peer has no surface");
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    (view, cx)
}

fn split_surface() -> PaneSurfaceFrame {
    let mut surface = mock_peer::surface(1);
    let FrameData { width, height, .. } = surface.frame;
    let first = width / 2;
    surface.panes = [(0, first + 1), (first + 1, width - first - 1)]
        .into_iter()
        .enumerate()
        .map(|(index, (x, width))| {
            let rect = SurfaceRect {
                x,
                y: 0,
                width,
                height,
            };
            PaneSurfacePane {
                pane_id: format!("pane-{index}"),
                content_revision: 1,
                rect,
                inner_rect: rect,
                scrollbar_rect: None,
                scroll: None,
                focused: index == 1,
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                alternate_screen_active: false,
                pixel_width: 0,
                pixel_height: 0,
            }
        })
        .collect();
    surface
}
