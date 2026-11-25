//! L66 link detection on pane frames: plain addresses, addresses wrapped
//! across rows, explicit OSC 8 hyperlinks, punctuation after an address,
//! wide characters, and the schemes and paths that open nothing.
use super::{Link, Refusal, Target, pane_hyperlink_at, pane_link_at, wrap::MAX_WRAPPED_ROWS};
use sheprd_client::protocol::{
    CellData, FrameData, PaneSurfaceFrame, PaneSurfacePane, SurfaceRect,
};
use unicode_width::UnicodeWidthChar;

#[path = "terminal_links_tests/core_open.rs"]
mod core_open;
#[path = "terminal_links_tests/hover.rs"]
mod hover;
#[path = "terminal_links_tests/marked.rs"]
mod marked;
#[path = "terminal_links_tests/resolve.rs"]
mod resolve;
#[path = "terminal_links_tests/tap.rs"]
mod tap;

const WIDTH: u16 = 40;
const HEIGHT: u16 = 6;

fn blank() -> CellData {
    CellData {
        symbol: " ".into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    }
}

/// A surface of one pane over a `WIDTH` x `HEIGHT` frame.
fn surface() -> PaneSurfaceFrame {
    let rect = SurfaceRect {
        x: 0,
        y: 0,
        width: WIDTH,
        height: HEIGHT,
    };
    PaneSurfaceFrame {
        boot_id: "boot".into(),
        projection_revision: 1,
        surface_revision: 1,
        frame: FrameData {
            width: WIDTH,
            height: HEIGHT,
            cells: vec![blank(); usize::from(WIDTH) * usize::from(HEIGHT)],
            cursor: None,
            hyperlinks: vec![],
            graphics: vec![],
        },
        panes: vec![pane("p1", rect)],
        splits: vec![],
        popup: None,
        graphics: Default::default(),
    }
}

fn pane(id: &str, rect: SurfaceRect) -> PaneSurfacePane {
    PaneSurfacePane {
        pane_id: id.into(),
        content_revision: 1,
        rect,
        inner_rect: rect,
        scrollbar_rect: None,
        scroll: None,
        focused: true,
        mouse_reporting: false,
        sgr_pixel_mouse: false,
        alternate_screen_active: false,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Writes `text` from (`column`, `row`) as Herdr 1 does: a wide glyph's
/// continuation cell is a blank with `skip` false, or empty when `empty`.
fn write(surface: &mut PaneSurfaceFrame, column: u16, row: u16, text: &str) -> u16 {
    write_as(surface, column, row, text, None, false)
}

fn write_as(
    surface: &mut PaneSurfaceFrame,
    column: u16,
    row: u16,
    text: &str,
    hyperlink: Option<u32>,
    empty: bool,
) -> u16 {
    let frame = &mut surface.frame;
    let mut at = usize::from(column);
    for ch in text.chars() {
        let width = ch.width().unwrap_or(1).max(1);
        let index = usize::from(row) * usize::from(frame.width) + at;
        frame.cells[index] = CellData {
            symbol: ch.to_string(),
            hyperlink,
            ..blank()
        };
        if width == 2 {
            frame.cells[index + 1] = CellData {
                symbol: if empty { String::new() } else { " ".into() },
                hyperlink,
                ..blank()
            };
        }
        at += width;
    }
    u16::try_from(at).unwrap_or(u16::MAX)
}

fn web(url: &str) -> Target {
    Target::Web(url.into())
}

/// A press read as on a Herdr 1 surface, which carries no soft-wrap marks.
fn at(surface: &PaneSurfaceFrame, column: u16, row: u16) -> Option<Link> {
    pane_link_at(surface, column, row, false)
}

#[test]
fn a_plain_address_opens_exactly_as_shown_from_any_of_its_cells() {
    let mut s = surface();
    let url = "https://example.com/a?b=1#c";
    write(&mut s, 0, 1, "see ");
    let end = write(&mut s, 4, 1, url);
    write(&mut s, end, 1, " next");
    for column in 4..end {
        let link = at(&s, column, 1).unwrap_or_else(|| panic!("column {column}"));
        assert_eq!(link.target, web(url));
        assert_eq!((link.row, link.columns.clone()), (1, 4..end));
        assert!(link.continued.is_empty());
    }
    assert_eq!(at(&s, 2, 1), None, "a word before it is no link");
    assert_eq!(at(&s, end + 2, 1), None, "a word after it is no link");
    assert_eq!(at(&s, 4, 2), None, "an empty row is no link");
    let http = "http://localhost:3000/";
    write(&mut s, 0, 3, http);
    assert_eq!(at(&s, 0, 3).map(|l| l.target), Some(web(http)));
}

#[test]
fn punctuation_after_an_address_stays_out_of_it() {
    for (text, url) in [
        ("go to https://example.com/x.", "https://example.com/x"),
        ("(see https://example.com/y).", "https://example.com/y"),
        ("a https://en.test/W_(x)), b", "https://en.test/W_(x)"),
        ("ok? https://example.com/z?!", "https://example.com/z"),
        ("<https://example.com/q>", "https://example.com/q"),
        ("\"https://example.com/r\";", "https://example.com/r"),
    ] {
        let mut s = surface();
        write(&mut s, 0, 0, text);
        let start = u16::try_from(text.find("https").unwrap_or(0)).unwrap_or(0);
        let link = at(&s, start + 9, 0).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(link.target, web(url), "{text}");
        let end = start + u16::try_from(url.len()).unwrap_or(0);
        assert_eq!(link.columns, start..end, "{text}");
        assert_eq!(at(&s, end, 0).map(|l| l.target), None, "{text}: past it");
    }
}

#[test]
fn an_address_read_across_the_right_edge_is_only_an_unproven_candidate() {
    let url = "https://example.com/wrapped/path/to/a/long/page?q=1";
    let mut s = surface();
    // The address starts at column 20 and goes on in row 2's first column,
    // as a terminal wraps; a line that ended at the edge looks the same.
    let (top, rest) = url.split_at(20);
    write(&mut s, 0, 1, "open");
    write(&mut s, 20, 1, top);
    let end = write(&mut s, 0, 2, rest);
    write(&mut s, end, 2, " then");
    for (column, row) in [(20, 1), (39, 1), (0, 2), (end - 1, 2)] {
        let link = at(&s, column, row).unwrap_or_else(|| panic!("({column}, {row})"));
        assert_eq!(link.target, web(url), "({column}, {row})");
        assert!(link.unproven, "({column}, {row}): opened as read");
        assert_eq!(
            link.rows().collect::<Vec<_>>(),
            vec![(1, 20..40), (2, 0..end)],
            "({column}, {row})"
        );
    }
    // A next row that starts with a blank is a new line: a wrapped address
    // would go on there with its next character. The row's address is whole.
    let mut s = surface();
    write(&mut s, 20, 1, top);
    write(&mut s, 2, 2, rest);
    let link = at(&s, 25, 1);
    assert_eq!(link.as_ref().map(|l| &l.target), Some(&web(top)));
    assert_eq!(link.map(|l| l.unproven), Some(false), "a certain line end");
    // Ending one short of the edge, it is complete.
    let mut s = surface();
    write(&mut s, 19, 1, &top[..20]);
    write(&mut s, 0, 2, "next line");
    let link = at(&s, 25, 1);
    assert_eq!(link.as_ref().map(|l| &l.target), Some(&web(&top[..20])));
    assert_eq!(link.map(|l| l.unproven), Some(false));
}

/// F1 (review 5fa66328 of PR 135): an address that ends exactly at the right
/// edge, followed by a new line whose first word starts in column 0, was
/// opened joined with that word: an address the screen never shows.
#[test]
fn an_address_ending_at_the_edge_is_never_joined_with_the_next_line_as_read() {
    let url = "https://example.com/ab";
    let mut s = surface();
    let start = WIDTH - u16::try_from(url.len()).unwrap_or(0);
    write(&mut s, start, 0, url);
    write(&mut s, 0, 1, "tail of another line");
    let link = at(&s, start + 3, 0).unwrap_or_else(|| panic!("the address"));
    assert!(
        link.unproven,
        "read as {:?} and opened as read",
        link.target
    );
    // At the pane's last row, what follows is not shown at all.
    let mut s = surface();
    write(&mut s, start, HEIGHT - 1, url);
    assert_eq!(
        at(&s, start + 3, HEIGHT - 1).map(|l| l.unproven),
        Some(true)
    );
}

/// F3: a wide glyph that does not fit in a row's last cell goes to the next
/// row, and both Herdr 1 (ghostty's spacer head) and Herdr 2 (alacritty's
/// leading wide-char spacer) leave a blank in that last cell. The address
/// there was read as complete and opened cut short.
#[test]
fn an_address_whose_wide_glyph_moved_to_the_next_row_is_unproven() {
    let head = "https://example.com/";
    let mut s = surface();
    // The address fills up to the second-to-last column; the last stays blank.
    let start = WIDTH - 1 - u16::try_from(head.len()).unwrap_or(0);
    write(&mut s, start, 0, head);
    write(&mut s, 0, 1, "日本 then");
    let link = at(&s, start + 3, 0).unwrap_or_else(|| panic!("the address"));
    assert_eq!(
        link.target,
        web("https://example.com/日本"),
        "the spacer is no text"
    );
    assert!(link.unproven);
    assert_eq!(
        link.rows().collect::<Vec<_>>(),
        vec![(0, start..WIDTH), (1, 0..4)],
    );
    // A narrow first character below is no such move: the row ended.
    let mut s = surface();
    write(&mut s, start, 0, head);
    write(&mut s, 0, 1, "ab then");
    let link = at(&s, start + 3, 0);
    assert_eq!(link.as_ref().map(|l| &l.target), Some(&web(head)));
    assert_eq!(link.map(|l| l.unproven), Some(false));
}

#[test]
fn an_address_read_past_the_row_limit_is_unproven() {
    let rows = MAX_WRAPPED_ROWS + 2;
    let mut s = surface();
    s.frame.height = rows;
    s.frame.cells = vec![blank(); usize::from(WIDTH) * usize::from(rows)];
    s.panes[0].inner_rect.height = rows;
    let url = format!(
        "https://example.com/{}",
        "x".repeat(usize::from(WIDTH) * 20)
    );
    for row in 0..rows {
        let from = usize::from(row) * usize::from(WIDTH);
        write(&mut s, 0, row, &url[from..from + usize::from(WIDTH)]);
    }
    assert_eq!(at(&s, 3, 0).map(|l| l.unproven), Some(true), "from the top");
    assert_eq!(at(&s, 3, rows - 1), None, "from the bottom: no scheme read");
}

#[test]
fn an_osc8_hyperlink_opens_its_web_target_across_its_cells() {
    let mut s = surface();
    s.frame.hyperlinks = vec!["https://example.com/docs?q=one#two".into()];
    // Wide cells in the link text, with both continuation shapes.
    let end = write_as(&mut s, 2, 0, "Docs 日本 link", Some(0), false);
    write_as(&mut s, 2, 1, "説明", Some(0), true);
    for column in 2..end {
        let link = at(&s, column, 0).unwrap_or_else(|| panic!("column {column}"));
        assert_eq!(link.target, web("https://example.com/docs?q=one#two"));
        assert_eq!(link.columns, 2..end, "column {column}");
    }
    assert!(pane_hyperlink_at(&s, 3, 1));
    assert_eq!(at(&s, 3, 1).map(|l| l.columns), Some(2..6), "continuation");
    assert_eq!(at(&s, end, 0), None);
    assert!(!pane_hyperlink_at(&s, end, 0));
}

#[test]
fn an_osc8_hyperlink_with_another_target_opens_nothing() {
    for (destination, refusal) in [
        ("mailto:a@example.com", Refusal::Scheme("mailto".into())),
        ("javascript:alert(1)", Refusal::Scheme("javascript".into())),
        ("ssh://host/x", Refusal::Scheme("ssh".into())),
        ("file:///etc/passwd", Refusal::Path),
        ("~/notes.txt", Refusal::Path),
        ("https://", Refusal::Malformed),
        ("https://example.com/a b", Refusal::Malformed),
        ("", Refusal::Malformed),
    ] {
        let mut s = surface();
        s.frame.hyperlinks = vec![destination.into()];
        write_as(&mut s, 0, 0, "click here", Some(0), false);
        let link = at(&s, 1, 0).unwrap_or_else(|| panic!("{destination}"));
        assert_eq!(link.target, Target::Refused(refusal), "{destination}");
    }
    // An index past the table: still a hyperlink, but none is read.
    let mut s = surface();
    write_as(&mut s, 0, 0, "dangling", Some(4), false);
    assert_eq!(at(&s, 1, 0), None);
    assert!(pane_hyperlink_at(&s, 1, 0));
}

#[test]
fn wide_characters_around_and_inside_an_address_keep_its_columns() {
    for empty in [false, true] {
        let mut s = surface();
        let start = write_as(&mut s, 0, 0, "日本語 ", None, empty);
        let url = "https://example.com/日本/x";
        let end = write_as(&mut s, start, 0, url, None, empty);
        write_as(&mut s, end, 0, " 漢字", None, empty);
        for column in start..end {
            let link = at(&s, column, 0).unwrap_or_else(|| panic!("{empty} {column}"));
            assert_eq!(link.target, web(url), "{empty} {column}");
            assert_eq!(link.columns, start..end, "{empty} {column}");
        }
        assert_eq!(at(&s, 1, 0), None, "{empty}: the wide word before");
        assert_eq!(at(&s, end + 1, 0), None, "{empty}: the wide word after");
    }
}

#[test]
fn other_schemes_and_paths_in_text_open_nothing_and_say_why() {
    for (text, refusal) in [
        ("mailto:qa@example.com", Refusal::Scheme("mailto".into())),
        ("ftp://files.example.com/a", Refusal::Scheme("ftp".into())),
        ("javascript:alert(1)", Refusal::Scheme("javascript".into())),
        ("file:///etc/hosts", Refusal::Path),
        ("/etc/hosts", Refusal::Path),
        ("~/notes.txt", Refusal::Path),
        ("./build/out.log", Refusal::Path),
        ("src/main.rs:12", Refusal::Path),
        ("HTTPS://", Refusal::Malformed),
    ] {
        let mut s = surface();
        write(&mut s, 1, 0, "see");
        write(&mut s, 5, 0, text);
        let link = at(&s, 6, 0).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(link.target, Target::Refused(refusal.clone()), "{text}");
        assert!(!refusal.reason().contains('\n'), "{text}: one line");
    }
    let mut s = surface();
    write(&mut s, 1, 0, "plain words, no link: a:b");
    for column in 1..26 {
        assert_eq!(at(&s, column, 0), None, "column {column}");
    }
    // A capitalised web scheme is still a web address.
    let mut s = surface();
    write(&mut s, 0, 0, "HTTPS://EXAMPLE.COM/A");
    assert_eq!(
        at(&s, 3, 0).map(|l| l.target),
        Some(web("HTTPS://EXAMPLE.COM/A"))
    );
}

#[test]
fn a_link_never_leaves_its_pane_and_a_popup_hides_links() {
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
    // The left pane's address runs to its edge; the right pane's text
    // beside it is another pane's, never the rest of the address.
    write(&mut s, 0, 0, "https://example.co/a");
    write(&mut s, 21, 0, "zz");
    write(&mut s, 0, 1, "tail");
    // The candidate joins the left pane's own next row, never the right
    // pane, and is unproven: it opens only as Herdr resolves it (F1).
    assert_eq!(
        at(&s, 3, 0).map(|l| (l.rows().collect::<Vec<_>>(), l.unproven)),
        Some((vec![(0, 0..20), (1, 0..4)], true))
    );
    assert_eq!(at(&s, 21, 0), None, "the right pane");
    assert_eq!(at(&s, 20, 0), None, "the line between panes");
    s.popup = Some(Box::new(sheprd_client::protocol::ClientShellPopupSurface {
        terminal_id: "popup".into(),
        title: String::new(),
        width: None,
        height: None,
        frame: s.frame.clone(),
        mouse_reporting: false,
        sgr_pixel_mouse: false,
        pixel_width: 0,
        pixel_height: 0,
    }));
    assert_eq!(at(&s, 3, 0), None, "under a popup");
    assert!(!pane_hyperlink_at(&s, 3, 0));
}

#[test]
fn concealed_text_is_no_link() {
    let mut s = surface();
    write(&mut s, 0, 0, "https://example.com/secret");
    for cell in &mut s.frame.cells[..26] {
        cell.modifier |= crate::terminal::HIDDEN;
    }
    assert_eq!(at(&s, 3, 0), None);
}
