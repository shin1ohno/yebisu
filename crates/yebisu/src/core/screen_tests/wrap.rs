//! #141: composition keeps each Herdr 2 pane's soft-wrap marks, so a wrapped
//! address reads whole in the composed frame, and a pane whose emulator
//! frame is not the pane's width yet (80 x 24 after a reconnect, until the
//! resize) is marked `UNMARKED`, as its clipped or padded rows prove nothing.
use super::support::{cell, saved, screen, size, wrap_row};
use crate::{
    core::screen::{Screen, publish_for},
    terminal_links::{SOFT_WRAP, Target, UNMARKED, pane_link_at, qa},
};
use sheprd_client::protocol::{PaneSurfaceFrame, SurfaceRect};

/// Writes `text` from column 0 of `row`, one cell per character.
fn put(screen: &mut Screen, row: u16, text: &str) {
    let Some(frame) = screen.frame.as_mut() else {
        panic!("fixture screen has a frame");
    };
    let start = usize::from(row) * usize::from(frame.width);
    for (index, ch) in text.chars().enumerate() {
        frame.cells[start + index] = cell(&ch.to_string());
    }
}

/// An address the emulator of `screen` wrapped from row 2 into row 3.
fn wrapped(screen: &mut Screen) -> String {
    let width = screen
        .frame
        .as_ref()
        .map_or(0, |frame| usize::from(frame.width));
    let url = format!("https://example.com/{}", "w".repeat(width - 5));
    let (top, rest) = url.split_at(width);
    put(screen, 2, top);
    put(screen, 3, &format!("{rest} then"));
    wrap_row(screen, 2);
    url
}

/// Two side-by-side panes, 40 columns each, with `left` drawn first.
fn composed(left: Screen) -> crate::Result<(PaneSurfaceFrame, SurfaceRect)> {
    let screens = vec![left, screen(" ", 40, 10)];
    let live = publish_for(&screens, &saved(1, false)?, size(81, 10)?, (1, 1, 0), None);
    let Some(surface) = live.surface.as_ref() else {
        panic!("published a surface");
    };
    let rect = surface.panes[0].inner_rect;
    assert_eq!(rect.width, 40, "the left pane's columns");
    Ok(((**surface).clone(), rect))
}

/// Whether the first cell of each of `rect`'s rows carries `UNMARKED`.
fn unmarked(surface: &PaneSurfaceFrame, rect: SurfaceRect) -> Vec<bool> {
    (rect.y..rect.y + rect.height)
        .map(|row| {
            let at = usize::from(row) * usize::from(surface.frame.width) + usize::from(rect.x);
            surface.frame.cells[at].modifier & UNMARKED != 0
        })
        .collect()
}

#[test]
fn composition_keeps_each_panes_marks() -> crate::Result<()> {
    let mut left = screen(" ", 40, 10);
    let url = wrapped(&mut left);
    let (surface, rect) = composed(left)?;
    let last = usize::from(rect.y + 2) * usize::from(surface.frame.width)
        + usize::from(rect.x + rect.width - 1);
    assert_ne!(
        surface.frame.cells[last].modifier & SOFT_WRAP,
        0,
        "the mark"
    );
    assert_eq!(unmarked(&surface, rect), vec![false; 10]);
    let Some(link) = pane_link_at(&surface, rect.x + 2, rect.y + 3, true) else {
        panic!("no link read");
    };
    assert_eq!(link.target, Target::Web(url));
    assert!(!link.unproven, "a proven wrap");
    assert_eq!(link.continued.len(), 1);
    Ok(())
}

#[test]
fn a_pane_not_yet_its_width_is_unmarked() -> crate::Result<()> {
    // Wider than its pane: the row is clipped and the mark with it, so the
    // last drawn cell has none and would "prove" a truncated line end.
    let mut wide = screen(" ", 50, 10);
    wrapped(&mut wide);
    let (surface, rect) = composed(wide)?;
    assert_eq!(unmarked(&surface, rect), vec![true; 10]);
    let link = pane_link_at(&surface, rect.x + 2, rect.y + 2, true);
    assert_eq!(link.as_ref().map(|l| l.unproven), Some(true), "{link:?}");
    // Narrower: padded, read as a frame without marks.
    let mut narrow = screen(" ", 30, 10);
    wrapped(&mut narrow);
    let (surface, rect) = composed(narrow)?;
    assert_eq!(unmarked(&surface, rect), vec![true; 10]);
    for (column, row) in [(2, 2), (2, 3)] {
        let (column, row) = (rect.x + column, rect.y + row);
        assert_eq!(
            pane_link_at(&surface, column, row, true),
            pane_link_at(&surface, column, row, false),
            "({column}, {row})"
        );
    }
    // The other pane, of its rect's width, is read with its marks.
    let right = surface.panes[1].inner_rect;
    assert_eq!(unmarked(&surface, right), vec![false; 10]);
    Ok(())
}


/// The pinned emulator marks its own wraps: the Herdr 2 fixture's address,
/// longer than the row, reads whole through composition. This fails if the
/// fork pin moves to a commit whose frames carry no mark (#141).
#[test]
fn the_pinned_emulator_marks_its_wraps() -> crate::Result<()> {
    assert_eq!(sheprd_terminal::SOFT_WRAP, SOFT_WRAP);
    let mut emulator = sheprd_terminal::Emulator::new(size(40, 10)?, 100);
    emulator.feed(qa::herdr2_output(40).as_bytes());
    let screens = vec![Screen {
        frame: Some(emulator.frame()),
        ..Screen::default()
    }];
    let live = publish_for(&screens, &saved(0, false)?, size(40, 10)?, (1, 1, 0), None);
    let Some(surface) = live.surface.as_ref() else {
        panic!("published a surface");
    };
    let Some(&(_, (column, row))) = qa::targets(surface)
        .iter()
        .find(|(name, _)| *name == "core_wrapped")
    else {
        panic!("the wrapped address is not on the frame");
    };
    let Some(link) = pane_link_at(surface, column, row, true) else {
        panic!("no link read");
    };
    let whole = format!("{}{}", qa::CORE_WRAPPED, "x".repeat(80));
    assert_eq!(link.target, Target::Web(whole));
    assert!(!link.unproven, "the emulator's marks prove the wrap");
    Ok(())
}
