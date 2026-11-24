//! A Herdr 1 split as the daemon lays it out with its defaults, for the
//! layout tests and, with `YEBISU_HERDR1_SPLIT_QA`, the Simulator peer.
//!
//! Herdr 1 defaults (`ui.pane_borders` auto, `pane_gaps`, `pane_outer_borders`
//! and `pane_scrollbars`) give every pane of a split a border on all four
//! sides. A pane on the primary screen keeps its last column inside the border
//! for the scrollbar. The first pane takes the rounded half
//! (herdr `src/layout.rs` `split_rect`, `src/ui/panes.rs` `apply_pane_chrome`
//! and `terminal_inner_rect_for`).
#[cfg(all(target_os = "ios", target_abi = "sim"))]
use sheprd_client::protocol::{CellData, FrameData, PaneSurfaceScrollMetrics};
use sheprd_client::protocol::{PaneSurfacePane, SurfaceRect};

/// Simulator only: answer each resize with a two-pane Herdr 1 split.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) const ENV: &str = "YEBISU_HERDR1_SPLIT_QA";

/// The panes of a two-pane split of a `cols` × `rows` frame. `alternate`
/// says which pane runs a program on the alternate screen.
pub(crate) fn panes(
    cols: u16,
    rows: u16,
    alternate: [bool; 2],
    stacked: bool,
) -> Vec<PaneSurfacePane> {
    let extent = if stacked { rows } else { cols };
    let first = extent.div_ceil(2);
    [(0, first), (first, extent.saturating_sub(first))]
        .into_iter()
        .zip(alternate)
        .zip(["w1:p1", "w1:p3"])
        .map(|(((start, length), alternate), pane_id)| {
            let rect = if stacked {
                SurfaceRect {
                    x: 0,
                    y: start,
                    width: cols,
                    height: length,
                }
            } else {
                SurfaceRect {
                    x: start,
                    y: 0,
                    width: length,
                    height: rows,
                }
            };
            let gutter = u16::from(!alternate);
            PaneSurfacePane {
                pane_id: pane_id.into(),
                content_revision: 1,
                rect,
                inner_rect: SurfaceRect {
                    x: rect.x.saturating_add(1),
                    y: rect.y.saturating_add(1),
                    width: rect.width.saturating_sub(2 + gutter),
                    height: rect.height.saturating_sub(2),
                },
                scrollbar_rect: None,
                scroll: None,
                focused: start == 0,
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                alternate_screen_active: alternate,
                pixel_width: 0,
                pixel_height: 0,
            }
        })
        .collect()
}

/// The owner's screenshot of 2026-11-25 as a frame: a shell with scrollback
/// on the primary screen beside a program on the alternate screen. Every
/// row is a column ruler to the pane's right edge, and the fourth row is in a
/// grey background to its end, like Claude Code's prompt row.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn herdr1_frame(frame: &mut FrameData) -> Vec<PaneSurfacePane> {
    let mut panes = panes(frame.width, frame.height, [false, true], false);
    for pane in &mut panes {
        paint(frame, pane);
        if !pane.alternate_screen_active {
            let inner = pane.inner_rect;
            pane.scrollbar_rect = Some(SurfaceRect {
                x: inner.x.saturating_add(inner.width),
                width: 1,
                ..inner
            });
            pane.scroll = Some(PaneSurfaceScrollMetrics {
                offset_from_bottom: 0,
                max_offset_from_bottom: u64::from(inner.height),
                viewport_rows: u64::from(inner.height),
            });
        }
    }
    panes
}

/// Rulers in the pane's cells, a scrollbar track in its gutter and Herdr's
/// border characters around them.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn paint(frame: &mut FrameData, pane: &PaneSurfacePane) {
    const RULER: &[u8] = b"1234567890";
    const GREY: u32 = 0x0237_3737;
    const TEXT: u32 = 0x02d8_dee9;
    const TRACK: u32 = 0x024c_566a;
    let (rect, inner) = (pane.rect, pane.inner_rect);
    let width = usize::from(frame.width);
    let mut put = |x: u16, y: u16, symbol: &str, fg: u32, bg: u32| {
        if let Some(cell) = frame.cells.get_mut(usize::from(y) * width + usize::from(x)) {
            *cell = CellData {
                symbol: symbol.into(),
                fg,
                bg,
                modifier: 0,
                skip: false,
                hyperlink: None,
            };
        }
    };
    let inner_end = inner.x.saturating_add(inner.width);
    let (right, bottom) = (
        rect.x.saturating_add(rect.width).saturating_sub(1),
        rect.y.saturating_add(rect.height).saturating_sub(1),
    );
    for y in rect.y..=bottom {
        for x in rect.x..=right {
            if y == rect.y || y == bottom {
                put(x, y, "─", TRACK, 0);
            } else if x == rect.x || x == right {
                put(x, y, "│", TRACK, 0);
            } else if (inner.x..inner_end).contains(&x) {
                let digit = RULER[usize::from(x - inner.x) % RULER.len()];
                let bg = if y == inner.y.saturating_add(3) {
                    GREY
                } else {
                    0
                };
                put(x, y, &char::from(digit).to_string(), TEXT, bg);
            } else if !pane.alternate_screen_active {
                put(x, y, "│", TRACK, 0);
            }
        }
    }
}
