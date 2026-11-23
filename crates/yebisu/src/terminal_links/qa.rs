//! The L66 Simulator fixture: with `YEBISU_URL_TAP_QA` at launch, the Herdr 1
//! wire peer and the Herdr 2 core peer show links of every kind, and
//! `qa.state` carries `url_targets`, the window point of each, so the XCTest
//! presses them where the app draws them.
use crate::app::Smoke;
use sheprd_client::protocol::{CellData, FrameData, PaneSurfaceFrame};
use unicode_width::UnicodeWidthChar;

/// The launch variable that turns the fixture on in a Simulator build.
pub(crate) const ENV: &str = "YEBISU_URL_TAP_QA";

/// Whether this process shows the fixture.
pub(crate) fn enabled() -> bool {
    cfg!(any(test, all(target_os = "ios", target_abi = "sim"))) && std::env::var_os(ENV).is_some()
}

/// `YEBISU_URL_TAP_QA=hover`: the pointer is taken to rest on the wrapped
/// address with Command held, so a Simulator screenshot shows the Command
/// underline, which XCTest cannot hover a pointer to show.
pub(crate) fn forced_hover() -> bool {
    enabled() && std::env::var(ENV).is_ok_and(|value| value == "hover")
}

/// The plain address on the Herdr 1 peer, followed by a full stop.
pub(crate) const PLAIN: &str = "https://example.com/l66?q=plain";
/// The address the Herdr 1 peer wraps from the right edge into the next row.
pub(crate) const WRAPPED: &str = "https://example.com/l66/wrapped/0123456789abcdef";
/// The part of `WRAPPED` on its first row, which ends at the right edge.
pub(crate) const WRAPPED_TOP: &str = "https://example.com/l66/wrappe";
/// The explicit hyperlink's target and the text it is drawn as.
pub(crate) const OSC8: &str = "https://example.com/l66/osc8";
pub(crate) const OSC8_TEXT: &str = "OSC 8 docs link";
pub(crate) const MAILTO: &str = "mailto:qa@example.com";
pub(crate) const PATH: &str = "/etc/hosts";
/// The Herdr 2 peer's plain address, explicit hyperlink and wrapped address.
pub(crate) const CORE_PLAIN: &str = "https://example.com/l66/core-plain";
pub(crate) const CORE_OSC8: &str = "https://example.com/l66/core-osc8";
pub(crate) const CORE_OSC8_TEXT: &str = "core OSC 8 link";
pub(crate) const CORE_WRAPPED: &str = "https://example.com/l66/core-wrapped/";

/// Writes `text` from (`column`, `row`), a wide character over two cells
/// with a blank continuation, as Herdr 1 full frames carry it.
fn put(frame: &mut FrameData, row: u16, column: u16, text: &str, hyperlink: Option<u32>) {
    let width = usize::from(frame.width);
    let mut at = usize::from(column);
    for ch in text.chars() {
        let span = UnicodeWidthChar::width(ch).unwrap_or(1).max(1);
        for (index, symbol) in [ch.to_string(), " ".into()]
            .into_iter()
            .take(span)
            .enumerate()
        {
            if at + index >= width {
                return;
            }
            if let Some(cell) = frame.cells.get_mut(usize::from(row) * width + at + index) {
                *cell = CellData {
                    symbol,
                    fg: 0x0200d8de,
                    bg: 0,
                    modifier: 0,
                    skip: false,
                    hyperlink,
                };
            }
        }
        at += span;
    }
}

/// Rows 12 to 17 of the Herdr 1 peer's frame: a plain address
/// after wide characters, with a full stop after it, one wrapped from the
/// right edge into the next row's first column, an explicit hyperlink,
/// another scheme and a path.
pub(crate) fn herdr1_fixture(frame: &mut FrameData) {
    // Rows 12 to 17 stay above a 19-row terminal (the software keyboard up)
    // and below the peer's own lines; a resized frame is written afresh.
    let width = usize::from(frame.width);
    for row in 12..18 {
        let start = row * width;
        for cell in frame.cells.iter_mut().skip(start).take(width) {
            *cell = CellData {
                symbol: " ".into(),
                fg: 0,
                bg: 0,
                modifier: 0,
                skip: false,
                hyperlink: None,
            };
        }
    }
    // Wide characters before it shift each later cell's text by bytes.
    put(frame, 12, 1, &format!("見て {PLAIN}. then text"), None);
    // The address starts `WRAPPED_TOP` before the right edge and goes on in
    // the next row's first column, as a terminal wraps it.
    let (top, rest) = WRAPPED.split_at(WRAPPED_TOP.len());
    let start = width.saturating_sub(top.len());
    put(frame, 13, 1, "wrap", None);
    put(frame, 13, u16::try_from(start).unwrap_or(0), top, None);
    put(frame, 14, 0, &format!("{rest} after"), None);
    let index = match frame.hyperlinks.iter().position(|link| link == OSC8) {
        Some(index) => index,
        None => {
            frame.hyperlinks.push(OSC8.into());
            frame.hyperlinks.len() - 1
        }
    };
    put(frame, 15, 1, OSC8_TEXT, u32::try_from(index).ok());
    put(frame, 16, 1, &format!("mail {MAILTO} here"), None);
    put(frame, 17, 1, &format!("path {PATH} here"), None);
    if let Some(cursor) = frame.cursor.as_mut() {
        cursor.y = cursor.y.max(18).min(frame.height.saturating_sub(1));
    }
}

/// What the Herdr 2 core peer prints after its banner in a pane `cols`
/// wide: a plain address, an OSC 8 hyperlink, and an address longer than
/// the row, which the terminal itself wraps.
pub(crate) fn herdr2_output(cols: u16) -> String {
    // Twice the width, so the address still wraps if the pane widens.
    let fill = "x".repeat(usize::from(cols) * 2);
    format!(
        "\r\n{CORE_PLAIN}\r\n\x1b]8;;{CORE_OSC8}\x1b\\{CORE_OSC8_TEXT}\x1b]8;;\x1b\\\r\n{CORE_WRAPPED}{fill}\r\n"
    )
}

/// The text of frame row `row` from `column`, one character per cell.
fn row_text(frame: &FrameData, row: u16) -> String {
    let width = usize::from(frame.width);
    frame
        .cells
        .iter()
        .skip(usize::from(row) * width)
        .take(width)
        .map(|cell| cell.symbol.chars().next().unwrap_or(' '))
        .collect()
}

/// The frame cells to press for each fixture link found in `surface`: the
/// fifth cell of a plain address, the second row of a wrapped one, and the
/// first cell of an explicit hyperlink.
pub(crate) fn targets(surface: &PaneSurfaceFrame) -> Vec<(&'static str, (u16, u16))> {
    let frame = &surface.frame;
    let mut found = Vec::new();
    for row in 0..frame.height {
        let text = row_text(frame, row);
        for (name, needle, offset) in [
            ("plain", PLAIN, 5),
            ("mailto", MAILTO, 3),
            ("path", PATH, 3),
            ("core_plain", CORE_PLAIN, 5),
            ("wrapped", WRAPPED_TOP, 2),
            ("core_wrapped", CORE_WRAPPED, 2),
        ] {
            // A wrapped address is published only while its row reaches
            // the right edge, so its second row is the address's.
            let fills = !text.ends_with(' ');
            if let Some(at) = text.find(needle)
                && (fills || !name.ends_with("wrapped"))
            {
                // A wrapped address is pressed on its second row.
                let cell = if name.ends_with("wrapped") {
                    (offset, row + 1)
                } else {
                    let column = text[..at].chars().count();
                    (
                        u16::try_from(column)
                            .unwrap_or(u16::MAX)
                            .saturating_add(offset),
                        row,
                    )
                };
                found.push((name, cell));
            }
        }
        let width = usize::from(frame.width);
        for (name, target) in [("osc8", OSC8), ("core_osc8", CORE_OSC8)] {
            let hit = (0..width).find(|&column| {
                frame
                    .cells
                    .get(usize::from(row) * width + column)
                    .and_then(|cell| cell.hyperlink)
                    .and_then(|index| frame.hyperlinks.get(index as usize))
                    .is_some_and(|link| link == target)
            });
            if let Some(column) = hit {
                found.push((name, (u16::try_from(column + 2).unwrap_or(u16::MAX), row)));
            }
        }
    }
    found
}

impl Smoke {
    /// Publishes `url_targets` in `qa.state`: each fixture link's window
    /// point, the centre of the cell to press.
    pub(crate) fn publish_link_targets(&mut self) {
        if !enabled() {
            return;
        }
        let Some((_, surface)) = self.terminal.coherent_frame() else {
            return;
        };
        let geometry = self.terminal.geometry;
        let panes = geometry.framed_panes(&surface);
        let point = |(column, row): (u16, u16)| {
            let (origin, rect_x, rect_y) = panes
                .iter()
                .find(|placement| crate::terminal::in_rect(placement.pane.inner_rect, column, row))
                .map_or((geometry.grid.origin, 0, 0), |placement| {
                    (
                        placement.body.origin,
                        placement.pane.inner_rect.x,
                        placement.pane.inner_rect.y,
                    )
                });
            let x = f32::from(origin.x) + (f32::from(column - rect_x) + 0.5) * geometry.cell_width;
            let y = f32::from(origin.y) + (f32::from(row - rect_y) + 0.5) * geometry.cell_height;
            [x.round(), y.round()]
        };
        let found = targets(&surface);
        let rest = found
            .iter()
            .find(|(name, _)| name.ends_with("wrapped"))
            .map(|&(_, cell)| point(cell));
        let mut targets: serde_json::Map<String, serde_json::Value> = found
            .into_iter()
            .map(|(name, cell)| (name.to_owned(), serde_json::json!(point(cell))))
            .collect();
        targets.insert(
            "surface".into(),
            serde_json::json!([surface.surface_revision, surface.boot_id]),
        );
        crate::qa_ax::report("url_targets", || serde_json::Value::Object(targets));
        if forced_hover()
            && let Some([x, y]) = rest
        {
            self.terminal
                .link_hover
                .rest_at(gpui::point(gpui::px(x), gpui::px(y)));
        }
    }
}
