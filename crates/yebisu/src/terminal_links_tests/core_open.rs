//! #141 through GPUI's real mouse dispatch on a Herdr 2 view: its frames
//! carry the emulator's soft-wrap marks, so a Command press opens a wrapped
//! address whole and an address ending at the edge as shown, and says why
//! for what the marks cannot settle. The view is the Herdr 1 wire peer's
//! (`tap::connected`, for a laid-out window and an input wire) switched to
//! Herdr 2, showing marked rows written here.
use super::tap::{command, connected, finger, press, received};
use crate::{
    app::Smoke,
    terminal::in_rect,
    terminal_links::{Refusal, SOFT_WRAP},
};
use gpui::{Entity, Modifiers, VisualTestContext, point, px};
use sheprd_client::protocol::{CellData, ClientMessage, FrameData};
use std::{ops::Range, sync::Arc, sync::mpsc};

/// The fixture's addresses and the frame cell to press on each.
struct Fixture {
    /// Wrapped from its first row into the next, both rows' cells.
    wrapped: String,
    wrapped_rows: Vec<(u16, Range<u16>)>,
    /// Ends at the right edge above a new line.
    edge: String,
    edge_at: (u16, u16),
    /// Read over 17 marked rows, pressed on its first.
    long_at: (u16, u16),
    /// Wrapped from the pane's first cell.
    top_left_at: (u16, u16),
}

fn put(frame: &mut FrameData, (column, row): (u16, u16), text: &str) {
    let width = usize::from(frame.width);
    for (index, ch) in text.chars().enumerate() {
        frame.cells[usize::from(row) * width + usize::from(column) + index] = CellData {
            symbol: ch.to_string(),
            fg: 0,
            bg: 0,
            modifier: 0,
            skip: false,
            hyperlink: None,
        };
    }
}

fn mark(frame: &mut FrameData, (column, row): (u16, u16)) {
    frame.cells[usize::from(row) * usize::from(frame.width) + usize::from(column)].modifier |=
        SOFT_WRAP;
}

/// Writes the fixture over the shown pane's rows, as a Herdr 2 frame
/// carries them, and returns where each address is.
fn core_fixture(frame: &mut FrameData, (x, y, width, height): (u16, u16, u16, u16)) -> Fixture {
    // The Herdr 1 peer's pane is 80 x 24: the fixture fills its 24 rows.
    assert!(height >= 24 && width >= 40, "pane {width} x {height}");
    for cell in &mut frame.cells {
        *cell = CellData {
            symbol: " ".into(),
            fg: 0,
            bg: 0,
            modifier: 0,
            skip: false,
            hyperlink: None,
        };
    }
    let last = x + width - 1;
    let fill = |count: u16| "x".repeat(usize::from(count));
    // Row y: an address from the pane's first cell, wrapped into row y + 1.
    let top_left = format!("https://example.com/141/top-left/{}", fill(width));
    let (head, tail) = top_left.split_at(usize::from(width));
    put(frame, (x, y), head);
    put(frame, (x, y + 1), &format!("{tail} after"));
    mark(frame, (last, y));
    // Rows y + 3 to y + 19, under a blank row: one address over 17 rows,
    // the first 16 marked.
    let long = format!("https://example.com/141/long/{}", fill(width * 17));
    for (row, chunk) in (y + 3..y + 20).zip(long.as_bytes().chunks(usize::from(width))) {
        put(
            frame,
            (x, row),
            std::str::from_utf8(chunk).unwrap_or_default(),
        );
        if row < y + 19 {
            mark(frame, (last, row));
        }
    }
    // Rows y + 20 and y + 21: wrapped from 20 cells before the edge.
    let wrapped = format!("https://example.com/141/wrapped/{}", fill(10));
    let (head, tail) = wrapped.split_at(20);
    put(frame, (last - 19, y + 20), head);
    put(frame, (x, y + 21), &format!("{tail} after"));
    mark(frame, (last, y + 20));
    let tail_end = x + u16::try_from(tail.len()).unwrap_or(0);
    // Row y + 22: ends at the edge; row y + 23 is a new line.
    let edge = "https://example.com/141/edge".to_owned();
    let start = last + 1 - u16::try_from(edge.len()).unwrap_or(0);
    put(frame, (start, y + 22), &edge);
    put(frame, (x, y + 23), "tail on a new line");
    Fixture {
        wrapped,
        wrapped_rows: vec![(y + 20, last - 19..last + 1), (y + 21, x..tail_end)],
        edge,
        edge_at: (start + 5, y + 22),
        long_at: (x + 3, y + 3),
        top_left_at: (x + 3, y),
    }
}

/// A connected view switched to Herdr 2, showing the fixture.
fn core_view(
    cx: &mut gpui::TestAppContext,
) -> (
    Entity<Smoke>,
    &mut VisualTestContext,
    mpsc::Receiver<ClientMessage>,
    Fixture,
) {
    let (view, cx, wire) = connected(cx);
    let fixture = view.update(cx, |this, cx| {
        this.terminal.core_mode = true;
        let Some(shown) = this.terminal.live.surface.as_ref() else {
            panic!("surface");
        };
        let mut surface = (**shown).clone();
        let pane = surface.panes.first().map_or(
            (0, 0, surface.frame.width, surface.frame.height),
            |pane| {
                let rect = pane.inner_rect;
                (rect.x, rect.y, rect.width, rect.height)
            },
        );
        let fixture = core_fixture(&mut surface.frame, pane);
        this.terminal.live.surface = Some(Arc::new(surface));
        cx.notify();
        fixture
    });
    cx.run_until_parked();
    (view, cx, wire, fixture)
}

/// The window point at the centre of frame cell (`column`, `row`), checked
/// against the view's own hit testing.
fn at(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    (column, row): (u16, u16),
) -> gpui::Point<gpui::Pixels> {
    view.update(cx, |this, _| {
        let Some((_, surface)) = this.terminal.coherent_frame() else {
            panic!("coherent frame");
        };
        let g = this.terminal.geometry;
        let (origin, x, y) = g
            .framed_panes(&surface)
            .iter()
            .find(|placement| in_rect(placement.pane.inner_rect, column, row))
            .map_or((g.grid.origin, 0, 0), |placement| {
                let rect = placement.pane.inner_rect;
                (placement.body.origin, rect.x, rect.y)
            });
        let position = origin
            + point(
                px((f32::from(column - x) + 0.5) * g.cell_width),
                px((f32::from(row - y) + 0.5) * g.cell_height),
            );
        assert_eq!(g.cell_at(&surface, position, None), Some((column, row)));
        position
    })
}

/// Input the terminal received, read over the Herdr 1 wire the view still
/// holds: the press under test has already happened.
fn sent(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &mpsc::Receiver<ClientMessage>,
) -> Vec<sheprd_client::protocol::ClientPaneInputEvent> {
    view.update(cx, |this, _| this.terminal.core_mode = false);
    received(view, cx, wire)
}

#[gpui::test]
fn a_marked_wrap_opens_whole_on_herdr2(cx: &mut gpui::TestAppContext) {
    let (view, cx, wire, fixture) = core_view(cx);
    for (row, columns) in fixture.wrapped_rows.clone() {
        for column in [columns.start, columns.end - 1] {
            let point = at(&view, cx, (column, row));
            press(cx, point, command());
            assert_eq!(
                cx.opened_url().as_deref(),
                Some(fixture.wrapped.as_str()),
                "({column}, {row})"
            );
            view.update(cx, |this, _| {
                assert_eq!(
                    this.terminal_status(),
                    format!("Opening {}", fixture.wrapped),
                    "({column}, {row})"
                );
                assert!(this.handoff.selection.is_none(), "no selection");
            });
        }
    }
    let point = at(&view, cx, fixture.edge_at);
    press(cx, point, command());
    assert_eq!(cx.opened_url().as_deref(), Some(fixture.edge.as_str()));
    view.update(cx, |this, _| {
        assert_eq!(this.terminal_status(), "Opening example.com in the browser");
    });
    assert_eq!(sent(&view, cx, &wire), [], "Command + click sent input");
}

#[gpui::test]
fn what_the_marks_cannot_settle_says_why(cx: &mut gpui::TestAppContext) {
    let (view, cx, wire, fixture) = core_view(cx);
    for (name, cell) in [("long", fixture.long_at), ("top-left", fixture.top_left_at)] {
        let point = at(&view, cx, cell);
        press(cx, point, command());
        assert_eq!(cx.opened_url(), None, "{name}: opened without proof");
        view.update(cx, |this, _| {
            assert_eq!(this.terminal_status(), Refusal::Unproven.reason(), "{name}");
        });
    }
    assert_eq!(sent(&view, cx, &wire), [], "a refused press sent input");
}

#[gpui::test]
fn the_command_underline_covers_every_marked_row(cx: &mut gpui::TestAppContext) {
    let (view, cx, _wire, fixture) = core_view(cx);
    let underlined = |view: &Entity<Smoke>, cx: &mut VisualTestContext| {
        view.update(cx, |this, _| this.terminal.link_hover.underlined().to_vec())
    };
    let (row, columns) = fixture.wrapped_rows[1].clone();
    let point = at(&view, cx, (columns.start + 1, row));
    cx.simulate_mouse_move(point, None, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(underlined(&view, cx), [], "underlined without Command");
    cx.simulate_modifiers_change(command());
    cx.run_until_parked();
    assert_eq!(underlined(&view, cx), fixture.wrapped_rows, "both rows");
    cx.simulate_modifiers_change(Modifiers::default());
    cx.run_until_parked();
    assert_eq!(underlined(&view, cx), [], "Command released");
    // A finger's tap shows no line and, without Command, opens nothing.
    finger(cx, point);
    assert_eq!(underlined(&view, cx), []);
    assert_eq!(cx.opened_url(), None, "a tap without Command opened");
}
