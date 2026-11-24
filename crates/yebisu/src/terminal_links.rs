//! Web addresses a terminal pane shows, opened by a tap or pointer click
//! with Command held (L66). Modified from shin1ohno/herdr-gpui
//! crates/sheprd-gpui/src/terminal/links.rs at
//! 398c132a06723a0cc1494842a34666d173ec0200 (`pane_link_at`,
//! `pane_hyperlink_at`, `frame_link`, `plain_url`) and links/wrap.rs.
//!
//! Terminal content is untrusted. Only an `http` or `https` address opens,
//! and the address opened is the text the pane shows, or the target an
//! explicit hyperlink names, byte for byte: nothing is resolved, completed or
//! run. Any other scheme, and any file path, opens nothing; the press says
//! why in the status line. Unlike the fork, which opens files and some other
//! schemes on the machine that runs the pane, this client runs on an iPad
//! and the panes run elsewhere, so those are refused here.
use crate::terminal::{HIDDEN, in_rect};
use sheprd_client::protocol::{CellData, FrameData, PaneSurfaceFrame};
use std::ops::Range;

mod hover;
mod open;
mod plain;
#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
pub(crate) mod qa;
pub(crate) mod resolve;
mod wrap;

use crate::handoff::cells::continuations;
pub(crate) use hover::Hover;

/// The most bytes of text read around a press, as in the fork.
const MAX_ROW_BYTES: usize = 32768;

/// What a link in a pane points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// An `http` or `https` address with a host, exactly as shown or named.
    Web(String),
    /// A link that opens nothing here, and why.
    Refused(Refusal),
}

/// Why a link opens nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// An address with another scheme, named in lower case, such as `mailto`.
    Scheme(String),
    /// A file path, or a `file:` address: the file is on the pane's host.
    Path,
    /// An explicit hyperlink whose target is no address this client reads.
    Malformed,
    /// A web address at a row edge that Herdr cannot say where it ends
    /// (Herdr 2, or a daemon without `pane.link.resolve`).
    Unproven,
    /// Herdr did not answer where the address ends, or answered an error.
    Unanswered,
    /// The screen changed before Herdr's answer arrived.
    Changed,
    /// Herdr found no web address under the press.
    NoAddress,
}

impl Refusal {
    /// The status line for a press on such a link: one line.
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Scheme(scheme) => {
                format!("Not opened: {scheme}: link · only http and https addresses open")
            }
            Self::Path => "Not opened: a file path is on the host · only web addresses open".into(),
            Self::Malformed => "Not opened: the link names no web address".into(),
            Self::Unproven => {
                "Not opened: the address meets the line edge and may go on · Herdr cannot say where it ends".into()
            }
            Self::Unanswered => "Not opened: Herdr did not say where the address ends".into(),
            Self::Changed => "Not opened: the screen changed · press again".into(),
            Self::NoAddress => "Not opened: Herdr found no web address there".into(),
        }
    }
}

/// A link read from a pane's rows, and the frame cells it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) target: Target,
    /// The first row the link covers, and its columns there.
    pub(crate) row: u16,
    pub(crate) columns: Range<u16>,
    /// The rows below `row` that a wrapped link goes on to, with the columns
    /// it covers on each; empty for a link on one row.
    pub(crate) continued: Vec<(u16, Range<u16>)>,
    /// Whether this web address meets a row edge where the terminal may have
    /// wrapped it: read across rows, or running into the right edge of a row
    /// whose next row could go on with it. The surface carries no soft-wrap
    /// flag, so such an address is only a candidate and opens only once
    /// Herdr says where it ends (`resolve`), never as read here.
    pub(crate) unproven: bool,
}

impl Link {
    /// Every row the link covers, top to bottom, with its columns there.
    pub(crate) fn rows(&self) -> impl Iterator<Item = (u16, Range<u16>)> + '_ {
        std::iter::once((self.row, self.columns.clone())).chain(self.continued.iter().cloned())
    }
}

/// The columns and rows of the pane that holds frame cell (`column`, `row`),
/// which a link never leaves. A popup is never a pane, so none while one is
/// shown. A surface without panes is one pane over the whole frame.
fn pane_area(
    surface: &PaneSurfaceFrame,
    column: u16,
    row: u16,
) -> Option<(Range<u16>, Range<u16>)> {
    if surface.popup.is_some() {
        return None;
    }
    let frame = &surface.frame;
    if surface.panes.is_empty() {
        return Some((0..frame.width, 0..frame.height));
    }
    let rect = surface
        .panes
        .iter()
        .find(|pane| in_rect(pane.inner_rect, column, row))?
        .inner_rect;
    Some((
        rect.x..rect.x.saturating_add(rect.width),
        rect.y..rect.y.saturating_add(rect.height),
    ))
}

/// The link of either kind, explicit or read from the text, at frame cell
/// (`column`, `row`) of a pane.
pub(crate) fn pane_link_at(surface: &PaneSurfaceFrame, column: u16, row: u16) -> Option<Link> {
    let (columns, rows) = pane_area(surface, column, row)?;
    frame_link(&surface.frame, column, row, columns, rows)
}

/// Whether an explicit hyperlink covers the pane cell at (`column`, `row`),
/// whatever its destination, even one `pane_link_at` cannot read.
pub(crate) fn pane_hyperlink_at(surface: &PaneSurfaceFrame, column: u16, row: u16) -> bool {
    let Some((columns, rows)) = pane_area(surface, column, row) else {
        return false;
    };
    rows.contains(&row)
        && link_cell(&surface.frame, column, row, columns.start, columns.end).is_some_and(
            |(cells, selected, source)| {
                cells[selected]
                    .hyperlink
                    .or(cells[source].hyperlink)
                    .is_some()
            },
        )
}

/// The text a cell shows: a blank for concealed or empty cells.
fn shown(cell: &CellData) -> &str {
    if cell.modifier & HIDDEN != 0 || cell.symbol.is_empty() {
        " "
    } else {
        cell.symbol.as_str()
    }
}

/// The row's cells from `start` to `end`, the index of the one at `column`,
/// and the index of the cell that draws it, which for a wide glyph's
/// continuation is the glyph's own; `None` off the row or on concealed text.
fn link_cell(
    frame: &FrameData,
    column: u16,
    row: u16,
    start: u16,
    end: u16,
) -> Option<(&[CellData], usize, usize)> {
    if column < start || column >= end || end > frame.width || row >= frame.height {
        return None;
    }
    let offset = usize::from(row) * usize::from(frame.width);
    let cells = frame
        .cells
        .get(offset + usize::from(start)..offset + usize::from(end))?;
    let selected = usize::from(column - start);
    let continuing = continuations(cells);
    let source = (0..=selected).rev().find(|&i| !continuing[i]).unwrap_or(0);
    (cells.get(source)?.modifier & HIDDEN == 0).then_some((cells, selected, source))
}

/// What an explicit hyperlink's target points at.
fn explicit_target(destination: &str) -> Target {
    if let Some(url) = plain::web(destination) {
        return Target::Web(url);
    }
    match plain::scheme(destination) {
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => Target::Refused(Refusal::Path),
        // A web scheme that failed validation names no address.
        Some(scheme)
            if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") =>
        {
            Target::Refused(Refusal::Malformed)
        }
        Some(scheme) => Target::Refused(Refusal::Scheme(scheme.to_ascii_lowercase())),
        // No scheme: a path as a program names it.
        None if !destination.is_empty() && !destination.chars().any(char::is_control) => {
            Target::Refused(Refusal::Path)
        }
        None => Target::Refused(Refusal::Malformed),
    }
}

/// The link at cell (`column`, `row`) of the pane that covers `columns` and
/// `rows` of `frame`, which it never leaves.
fn frame_link(
    frame: &FrameData,
    column: u16,
    row: u16,
    columns: Range<u16>,
    rows: Range<u16>,
) -> Option<Link> {
    if !rows.contains(&row) {
        return None;
    }
    let (cells, selected, source) = link_cell(frame, column, row, columns.start, columns.end)?;
    let left = columns.start;
    // Cell indexes fit the row, whose width is a `u16`.
    let span = |cells: Range<usize>| left + cells.start as u16..left + cells.end as u16;
    // An explicit link is authoritative, even when its target is refused.
    if let Some(index) = cells[selected].hyperlink.or(cells[source].hyperlink) {
        let destination = frame.hyperlinks.get(index as usize)?;
        let continuing = continuations(cells);
        let on_link = |i: &usize| continuing[*i] || cells[*i].hyperlink == Some(index);
        let first = (0..=source)
            .rev()
            .take_while(on_link)
            .last()
            .unwrap_or(source);
        let last = (selected..cells.len())
            .take_while(on_link)
            .last()
            .unwrap_or(selected);
        return Some(Link {
            target: explicit_target(destination),
            row,
            columns: span(first..last + 1),
            continued: Vec::new(),
            unproven: false,
        });
    }
    // Most rows end their line before the right edge and are read alone;
    // an address that runs into the edge, or starts a row under one that
    // does, is read with the rows it may span, as one line: a candidate.
    let area = wrap::Area::new(frame, columns, rows);
    let lines = area.around(row, source);
    let run = area.read(lines.rows.clone())?;
    let found = plain::read(&run.text, run.start_of(row, source)?)?;
    let segments = run.segments(&found.range);
    // The address may go on past what was read: across a row edge the
    // terminal may or may not have wrapped, to the right edge of a row whose
    // next row is not certainly a new line, or above the rows read.
    let unproven = matches!(found.target, Target::Web(_))
        && (segments.len() > 1
            || (found.open && !area.ends_line(*lines.rows.end()))
            || (found.range.start == 0 && lines.cut_above));
    let mut segments = segments.into_iter();
    let (row, columns) = segments.next()?;
    Some(Link {
        target: found.target,
        row,
        columns,
        continued: segments.collect(),
        unproven,
    })
}

#[cfg(test)]
#[path = "terminal_links_tests.rs"]
mod tests;
