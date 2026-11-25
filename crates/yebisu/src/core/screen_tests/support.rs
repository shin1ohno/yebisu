//! Fixtures: pane screens with one letter per pane, saved state for two
//! panes, and a measured terminal column.
use crate::{core::screen::Screen, core::store::Saved, shell::single_pane::ColumnMetrics};
use sheprd_backend::{FrameData, Size, TerminalId};
use sheprd_client::protocol::{CellData, CursorState};

pub(super) fn cell(symbol: &str) -> CellData {
    CellData {
        symbol: symbol.into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    }
}

/// A pane screen of `cols` x `rows` cells that all show `symbol`, with the
/// cursor at its own (0, 0).
pub(super) fn screen(symbol: &str, cols: u16, rows: u16) -> Screen {
    Screen {
        frame: Some(FrameData {
            width: cols,
            height: rows,
            cells: vec![cell(symbol); usize::from(cols) * usize::from(rows)],
            cursor: Some(CursorState {
                x: 0,
                y: 0,
                visible: true,
                shape: 1,
            }),
            hyperlinks: vec![],
            graphics: vec![],
        }),
        ..Screen::default()
    }
}

/// Marks `row` of `screen` as one its emulator wrapped into the next (#141):
/// `SOFT_WRAP` on the row's last cell, as sheprd-terminal sets it.
pub(super) fn wrap_row(screen: &mut Screen, row: u16) {
    let Some(frame) = screen.frame.as_mut() else {
        panic!("fixture screen has a frame");
    };
    let at = (usize::from(row) + 1) * usize::from(frame.width) - 1;
    frame.cells[at].modifier |= crate::terminal_links::SOFT_WRAP;
}

pub(super) fn saved(active: usize, vertical: bool) -> crate::Result<Saved> {
    Ok(Saved {
        ids: vec![TerminalId::new("t1")?, TerminalId::new("t2")?],
        active,
        vertical,
    })
}

pub(super) fn size(cols: u16, rows: u16) -> crate::Result<Size> {
    Ok(Size::new(cols, rows)?)
}

/// A column of `width` x `height` points with `cell_width` x `cell_height`
/// point cells.
pub(super) fn column(width: f32, height: f32, cell: (f32, f32)) -> ColumnMetrics {
    ColumnMetrics {
        width,
        height,
        cell_width: cell.0,
        cell_height: cell.1,
        focus_only: false,
    }
}
