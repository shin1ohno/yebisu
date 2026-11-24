//! #141: Herdr 2 frames carry the emulator's soft-wrap mark, `SOFT_WRAP` on
//! the last cell of a row it wrapped, which proves both a wrap and a line
//! end. Only what the mark cannot settle stays a candidate: an address at the
//! pane's top-left, one read past `MAX_WRAPPED_ROWS` rows, and a pane the
//! composer marked `UNMARKED`. Herdr 1 surfaces never read the mark.
use super::*;
use crate::terminal_links::{SOFT_WRAP, UNMARKED};

/// Sets `SOFT_WRAP` on frame cell (`column`, `row`).
fn mark_at(s: &mut PaneSurfaceFrame, column: u16, row: u16) {
    let index = usize::from(row) * usize::from(s.frame.width) + usize::from(column);
    s.frame.cells[index].modifier |= SOFT_WRAP;
}

/// Marks `row` as one the emulator wrapped: its last cell carries the mark.
fn mark(s: &mut PaneSurfaceFrame, row: u16) {
    mark_at(s, s.frame.width - 1, row);
}

/// A press read as on a Herdr 2 frame.
fn core_at(s: &PaneSurfaceFrame, column: u16, row: u16) -> Option<Link> {
    pane_link_at(s, column, row, true)
}

/// A one-pane surface `rows` high.
fn tall(rows: u16) -> PaneSurfaceFrame {
    let mut s = surface();
    s.frame.height = rows;
    s.frame.cells = vec![blank(); usize::from(WIDTH) * usize::from(rows)];
    s.panes[0].rect.height = rows;
    s.panes[0].inner_rect.height = rows;
    s
}

/// Writes `url` from column 0 of `first`, a full row at a time.
fn write_rows(s: &mut PaneSurfaceFrame, first: u16, url: &str) {
    for (row, chunk) in (first..).zip(url.as_bytes().chunks(usize::from(WIDTH))) {
        write(s, 0, row, std::str::from_utf8(chunk).unwrap_or_default());
    }
}

fn rows_of(link: &Link) -> Vec<(u16, std::ops::Range<u16>)> {
    link.rows().collect()
}

/// The fixture of `an_address_read_across_the_right_edge_is_only_an_unproven_candidate`:
/// an address from column 20 of row 1 going on in row 2, and where it ends.
fn wrapped() -> (PaneSurfaceFrame, &'static str, u16) {
    let url = "https://example.com/wrapped/path/to/a/long/page?q=1";
    let mut s = surface();
    let (top, rest) = url.split_at(20);
    write(&mut s, 0, 1, "open");
    write(&mut s, 20, 1, top);
    let end = write(&mut s, 0, 2, rest);
    write(&mut s, end, 2, " then");
    (s, url, end)
}

#[test]
fn a_marked_wrap_opens_whole_from_either_row() {
    let (mut s, url, end) = wrapped();
    mark(&mut s, 1);
    for (column, row) in [(20, 1), (39, 1), (0, 2), (end - 1, 2)] {
        let link = core_at(&s, column, row).unwrap_or_else(|| panic!("({column}, {row})"));
        assert_eq!(link.target, web(url), "({column}, {row})");
        assert!(!link.unproven, "({column}, {row}): a proven wrap");
        assert_eq!(rows_of(&link), vec![(1, 20..40), (2, 0..end)]);
    }
}

#[test]
fn a_marked_wrap_of_sixteen_rows_opens_whole() {
    let mut s = tall(MAX_WRAPPED_ROWS + 2);
    // Rows 1 to 16 under a blank row 0: 15 full rows and 10 cells.
    let url = format!(
        "https://example.com/{}",
        "x".repeat(usize::from(WIDTH) * 15 + 10 - 20)
    );
    write_rows(&mut s, 1, &url);
    for row in 1..MAX_WRAPPED_ROWS {
        mark(&mut s, row);
    }
    for row in [1, 8, MAX_WRAPPED_ROWS] {
        let link = core_at(&s, 3, row).unwrap_or_else(|| panic!("row {row}"));
        assert_eq!(link.target, web(&url), "row {row}");
        assert!(!link.unproven, "row {row}");
        assert_eq!(link.continued.len(), usize::from(MAX_WRAPPED_ROWS) - 1);
    }
}

#[test]
fn seventeen_marked_rows_stay_a_candidate() {
    let mut s = tall(MAX_WRAPPED_ROWS + 3);
    // Rows 1 to 17 under a blank row 0, so the pane's top is not the reason.
    let url = format!(
        "https://example.com/{}",
        "x".repeat(usize::from(WIDTH) * 16 + 10 - 20)
    );
    write_rows(&mut s, 1, &url);
    for row in 1..=MAX_WRAPPED_ROWS {
        mark(&mut s, row);
    }
    let link = core_at(&s, 3, 1).unwrap_or_else(|| panic!("the address"));
    assert!(matches!(link.target, Target::Web(_)));
    assert!(link.unproven, "the 16th row read still wraps");
    // From the bottom the 16 rows read hold no scheme, as today (R5).
    assert_eq!(core_at(&s, 3, MAX_WRAPPED_ROWS + 1), None);
}

#[test]
fn an_address_ending_at_the_edge_opens_as_shown_when_unmarked_there() {
    let url = "https://example.com/ab";
    let start = WIDTH - u16::try_from(url.len()).unwrap_or(0);
    let mut s = surface();
    write(&mut s, start, 0, url);
    write(&mut s, 0, 1, "tail of another line");
    let link = core_at(&s, start + 3, 0).unwrap_or_else(|| panic!("the address"));
    assert_eq!(link.target, web(url), "never joined with `tail`");
    assert!(link.continued.is_empty());
    assert!(!link.unproven, "no mark: a proven line end");
    // At the pane's last row, the mark alone says whether it goes on.
    let mut s = surface();
    write(&mut s, start, HEIGHT - 1, url);
    let last = |s: &PaneSurfaceFrame| core_at(s, start + 3, HEIGHT - 1).map(|l| l.unproven);
    assert_eq!(last(&s), Some(false));
    mark(&mut s, HEIGHT - 1);
    assert_eq!(last(&s), Some(true), "wraps into a row not shown");
}

#[test]
fn a_marked_leading_spacer_is_no_text() {
    let head = "https://example.com/";
    let start = WIDTH - 1 - u16::try_from(head.len()).unwrap_or(0);
    let fixture = || {
        let mut s = surface();
        write(&mut s, start, 0, head);
        write(&mut s, 0, 1, "日本 then");
        s
    };
    let mut s = fixture();
    mark(&mut s, 0);
    let link = core_at(&s, start + 3, 0).unwrap_or_else(|| panic!("the address"));
    assert_eq!(link.target, web("https://example.com/日本"));
    assert!(!link.unproven);
    assert_eq!(rows_of(&link), vec![(0, start..WIDTH), (1, 0..4)]);
    // Without the mark the row ended in a real blank: the address is whole.
    let link = core_at(&fixture(), start + 3, 0);
    assert_eq!(link.as_ref().map(|l| &l.target), Some(&web(head)));
    assert_eq!(link.map(|l| l.unproven), Some(false));
}

#[test]
fn a_wide_glyph_ending_a_marked_row_keeps_its_spacer_mark() {
    // Herdr 2's shape: the glyph's second cell is `skip` with no symbol, and
    // alacritty puts the wrap flag on it, the cell the cursor wrapped from.
    let mut s = surface();
    let url = "https://example.com/日";
    let end = write_as(&mut s, WIDTH - 22, 0, url, None, true);
    assert_eq!(end, WIDTH);
    s.frame.cells[usize::from(WIDTH) - 1].skip = true;
    mark(&mut s, 0);
    write(&mut s, 0, 1, "abc then");
    let link = core_at(&s, WIDTH - 20, 0).unwrap_or_else(|| panic!("the address"));
    assert_eq!(link.target, web("https://example.com/日abc"));
    assert!(!link.unproven);
    assert_eq!(rows_of(&link), vec![(0, WIDTH - 22..WIDTH), (1, 0..3)]);
}

#[test]
fn an_address_at_the_panes_top_left_keeps_todays_reading() {
    let url = format!("https://example.com/{}", "y".repeat(30));
    // From the pane's first cell, it may go on from a row the frame does not
    // show: refused as today, whichever row is pressed.
    let mut s = surface();
    write_rows(&mut s, 0, &url);
    write(&mut s, 10, 1, " then");
    mark(&mut s, 0);
    for (column, row) in [(3, 0), (2, 1)] {
        assert_eq!(
            core_at(&s, column, row).map(|l| l.unproven),
            Some(true),
            "({column}, {row})"
        );
        assert_eq!(core_at(&s, column, row), at(&s, column, row));
    }
    // One row ending before the edge opens, as today.
    let mut s = surface();
    write(&mut s, 0, 0, "https://example.com/one then");
    let link = core_at(&s, 3, 0);
    assert_eq!(
        link.as_ref().map(|l| &l.target),
        Some(&web("https://example.com/one"))
    );
    assert_eq!(link.map(|l| l.unproven), Some(false));
    // The same wrapped address one row down opens whole.
    let mut s = surface();
    write_rows(&mut s, 1, &url);
    write(&mut s, 10, 2, " then");
    mark(&mut s, 1);
    let link = core_at(&s, 2, 2).unwrap_or_else(|| panic!("the address"));
    assert_eq!(link.target, web(&url));
    assert!(!link.unproven);
    assert_eq!(rows_of(&link), vec![(1, 0..40), (2, 0..10)]);
}

#[test]
fn an_unresized_pane_proves_nothing() {
    let (mut s, _, end) = wrapped();
    mark(&mut s, 1);
    for row in 0..HEIGHT {
        s.frame.cells[usize::from(row) * usize::from(WIDTH)].modifier |= UNMARKED;
    }
    for (column, row) in [(20, 1), (0, 2), (end - 1, 2)] {
        let link = core_at(&s, column, row);
        assert_eq!(
            link.as_ref().map(|l| l.unproven),
            Some(true),
            "({column}, {row})"
        );
        assert_eq!(link, at(&s, column, row), "read as a frame without marks");
    }
}

#[test]
fn herdr_one_frames_read_as_today() {
    let (plain, _, _) = wrapped();
    let mut marked = plain.clone();
    mark(&mut marked, 1);
    // The edge address of F1 and the wide glyph of F3, marked.
    let edge = |marked: bool| {
        let mut s = surface();
        write(&mut s, 18, 0, "https://example.com/ab");
        write(&mut s, 0, 1, "tail of another line");
        if marked {
            mark(&mut s, 0);
        }
        s
    };
    let spacer = |marked: bool| {
        let mut s = surface();
        write(&mut s, 19, 0, "https://example.com/");
        write(&mut s, 0, 1, "日本 then");
        if marked {
            mark(&mut s, 0);
        }
        s
    };
    for (name, without, with, (column, row)) in [
        ("wrapped", plain, marked, (20, 1)),
        ("edge", edge(false), edge(true), (21, 0)),
        ("spacer", spacer(false), spacer(true), (22, 0)),
    ] {
        let today = at(&without, column, row);
        assert_eq!(today.as_ref().map(|l| l.unproven), Some(true), "{name}");
        assert_eq!(at(&with, column, row), today, "{name}: the mark was read");
    }
}

#[test]
fn marks_never_join_another_pane() {
    let mut s = surface();
    let left = SurfaceRect {
        x: 0,
        y: 0,
        width: 20,
        height: HEIGHT,
    };
    let right = SurfaceRect {
        x: 21,
        y: 0,
        width: 19,
        height: HEIGHT,
    };
    s.panes = vec![pane("left", left), pane("right", right)];
    // Below the pane's first row, which an address may go on from.
    write(&mut s, 0, 1, "https://example.co/a");
    write(&mut s, 21, 1, "zz");
    write(&mut s, 0, 2, "tail");
    // The left pane's own last cell carries its wrap.
    mark_at(&mut s, 19, 1);
    let link = core_at(&s, 3, 1).unwrap_or_else(|| panic!("the left address"));
    assert_eq!(link.target, web("https://example.co/atail"));
    assert_eq!(rows_of(&link), vec![(1, 0..20), (2, 0..4)]);
    assert!(!link.unproven);
    assert_eq!(core_at(&s, 21, 1), None, "the right pane");
    assert_eq!(core_at(&s, 20, 1), None, "the line between panes");
}
