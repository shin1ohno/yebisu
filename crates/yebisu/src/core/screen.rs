use crate::shell::herdr2_frame::panes::{PaneRect, frame_lines, pane_rects};
use crate::{
    layout::{PaneMode, herdr2_pane_mode},
    live::Live,
    shell::single_pane::ColumnMetrics,
};
use sheprd_backend::{FrameData, KeyReporting, ScrollPosition, Size, SurfaceUpdate};
use sheprd_client::protocol::{
    CellData, ClientShellPane, ClientShellSnapshot, PaneSurfaceFrame, PaneSurfacePane,
    PaneSurfaceScrollMetrics, SurfaceRect,
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Screen {
    pub frame: Option<FrameData>,
    pub keys: KeyReporting,
    pub ended: bool,
    pub title: Option<String>,
    /// Where the view is in the terminal's history (issue #137 PR 6).
    pub scroll: Option<ScrollPosition>,
    /// Whether the program reads the mouse, so the wheel reaches it.
    pub mouse_reporting: bool,
}
impl Screen {
    pub fn apply(&mut self, update: SurfaceUpdate) {
        match update {
            SurfaceUpdate::Frame(frame) => self.frame = Some(frame),
            SurfaceUpdate::Rows { cursor, rows } => {
                let Some(frame) = &mut self.frame else {
                    return;
                };
                for row in rows {
                    if row.x >= frame.width || row.y >= frame.height {
                        continue;
                    }
                    let start = usize::from(row.y) * usize::from(frame.width) + usize::from(row.x);
                    let count = usize::from(frame.width - row.x).min(row.cells.len());
                    if let Some(slots) = frame.cells.get_mut(start..start + count) {
                        slots.clone_from_slice(&row.cells[..count]);
                    }
                }
                frame.cursor = cursor;
            }
            SurfaceUpdate::KeyReporting(keys) => self.keys = keys,
            SurfaceUpdate::MouseReporting(on) => self.mouse_reporting = on,
            SurfaceUpdate::Scroll(position) => self.scroll = Some(position),
            SurfaceUpdate::Ended(_) => self.ended = true,
            SurfaceUpdate::Title(title) => {
                self.title = (!title.trim().is_empty()).then_some(title);
            }
            _ => {}
        }
    }
}
/// Whether the panes cannot share the column (plan v2 6): narrower than two
/// panes need, or divided so far that a pane would fall under the 280 x 206 pt
/// minimum. The drawn pane then gets the whole column.
///
/// Each pane's size is L8's `pane_rects` rect, which counts the one line
/// cell between neighbours (plan v2 7.14), so this rule, the size sent and
/// the rect drawn agree. A frame too small to hold the panes and their lines
/// is one pane too.
fn is_single(size: Size, count: usize, vertical: bool, column: Option<ColumnMetrics>) -> bool {
    let rects = pane_rects(size.cols(), size.rows(), count, vertical);
    if count >= 2 && rects.len() < count {
        return true;
    }
    let Some(column) = column else {
        return false;
    };
    if herdr2_pane_mode(column.width, count, column.focus_only) == PaneMode::Single {
        return true;
    }
    if count > 1 && !column.equal_panes_fit(count, vertical) {
        return true;
    }
    let (least_cols, least_rows) = column.minimum_cells();
    let Some(each) = rects.first() else {
        return true;
    };
    if vertical {
        each.rows < least_rows
    } else {
        each.cols < least_cols
    }
}
/// The size one terminal is given (plan v2 6.3, Herdr 2): the whole column
/// while one pane is drawn, otherwise the column divided between the panes.
/// `column` is the worker's read of the UI's column mailbox.
pub(super) fn pane_size(
    size: Size,
    count: usize,
    vertical: bool,
    column: Option<ColumnMetrics>,
) -> crate::Result<Size> {
    let sizes = pane_sizes(size, count, vertical, 0, column)?;
    sizes
        .into_iter()
        .flatten()
        .next()
        .ok_or(crate::Error::CoreState)
}
pub(super) fn pane_size_for(
    size: Size,
    count: usize,
    vertical: bool,
    column: Option<ColumnMetrics>,
) -> crate::Result<Size> {
    let count = count.max(1);
    if is_single(size, count, vertical, column) {
        return Ok(size);
    }
    // Every pane has the same rect; the line cells are not part of it.
    let each = pane_rects(size.cols(), size.rows(), count, vertical)
        .first()
        .copied()
        .ok_or(crate::Error::CoreState)?;
    Ok(Size::new(each.cols, each.rows)?)
}
/// What to send each terminal. A pane hidden by the one-pane rule gets `None`:
/// it keeps the size it last had (plan v2 6.3), so it does not reflow every
/// time the column crosses 568 pt.
pub(super) fn pane_sizes(
    size: Size,
    count: usize,
    vertical: bool,
    active: usize,
    column: Option<ColumnMetrics>,
) -> crate::Result<Vec<Option<Size>>> {
    let count = count.max(1);
    let sent = pane_size_for(size, count, vertical, column)?;
    let hidden = is_single(size, count, vertical, column);
    let sizes: Vec<Option<Size>> = (0..count)
        .map(|index| (!hidden || index == active).then_some(sent))
        .collect();
    #[cfg(test)]
    probe::record(size, count, active, &sizes);
    Ok(sizes)
}
pub(super) fn publish(
    screens: &[Screen],
    saved: &super::store::Saved,
    size: Size,
    generation: u64,
    revision: u64,
    epoch: u64,
    column: Option<ColumnMetrics>,
) -> Live {
    publish_for(screens, saved, size, (generation, revision, epoch), column)
}
/// `publish` for an explicit column measurement. A column too narrow for the
/// panes draws only `saved.active`, over the whole size.
pub(super) fn publish_for(
    screens: &[Screen],
    saved: &super::store::Saved,
    size: Size,
    (generation, revision, epoch): (u64, u64, u64),
    column: Option<ColumnMetrics>,
) -> Live {
    let single = is_single(size, screens.len(), saved.vertical, column);
    let blank = CellData {
        symbol: " ".into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    };
    let mut frame = FrameData {
        width: size.cols(),
        height: size.rows(),
        cells: vec![blank; usize::from(size.cols()) * usize::from(size.rows())],
        cursor: None,
        hyperlinks: vec![],
        graphics: vec![],
    };
    // One rect per pane (L8), or the whole frame for the one drawn pane.
    let rects = if single {
        vec![
            PaneRect {
                x: 0,
                y: 0,
                cols: size.cols().max(1),
                rows: size.rows().max(1),
            };
            screens.len()
        ]
    } else {
        pane_rects(size.cols(), size.rows(), screens.len(), saved.vertical)
    };
    if !single {
        for line in frame_lines(&rects, saved.vertical, saved.active) {
            let at = usize::from(line.y) * usize::from(size.cols()) + usize::from(line.x);
            if let Some(cell) = frame.cells.get_mut(at) {
                cell.symbol = line.symbol.into();
                cell.fg = line.fg;
            }
        }
    }
    for (index, screen) in screens.iter().enumerate() {
        let Some(source) = &screen.frame else {
            continue;
        };
        if single && index != saved.active {
            continue;
        }
        let Some(rect) = rects.get(index) else {
            continue;
        };
        let (x, y) = (rect.x, rect.y);
        // Each terminal numbers its hyperlinks into its own table; the
        // merged frame keeps one table, so this pane's links move past the
        // ones before it (L66). A cell naming no entry links nothing.
        let links = u32::try_from(frame.hyperlinks.len()).unwrap_or(u32::MAX);
        frame.hyperlinks.extend(source.hyperlinks.iter().cloned());
        let relink = |link: Option<u32>| {
            link.filter(|&index| (index as usize) < source.hyperlinks.len())
                .and_then(|index| index.checked_add(links))
        };
        // Clip to the pane's rect: it never leaves the frame (pane_rects).
        for row in 0..source
            .height
            .min(rect.rows)
            .min(size.rows().saturating_sub(y))
        {
            for col in 0..source
                .width
                .min(rect.cols)
                .min(size.cols().saturating_sub(x))
            {
                if let Some(cell) = source
                    .cells
                    .get(usize::from(row) * usize::from(source.width) + usize::from(col))
                {
                    let mut cell = cell.clone();
                    cell.hyperlink = relink(cell.hyperlink);
                    frame.cells
                        [usize::from(y + row) * usize::from(size.cols()) + usize::from(x + col)] =
                        cell;
                }
            }
        }
        // An emulator frame of another width (Herdr 2's first frames after a
        // reconnect are 80 x 24 until the resize) is clipped or padded, so
        // its last drawn column is not its last column and its soft-wrap
        // marks prove nothing: the link reader reads this pane unmarked.
        if source.width != rect.cols {
            for row in 0..rect.rows.min(size.rows().saturating_sub(y)) {
                let at = usize::from(y + row) * usize::from(size.cols()) + usize::from(x);
                if let Some(cell) = frame.cells.get_mut(at) {
                    cell.modifier |= crate::terminal_links::UNMARKED;
                }
            }
        }
        if index == saved.active && !screen.ended {
            frame.cursor = source.cursor.clone().map(|mut cursor| {
                cursor.x += x;
                cursor.y += y;
                cursor
            });
        }
    }
    crate::qa_ax::core::layout(screens.len(), saved.active, saved.vertical, &saved.ids);
    let boot = format!("core-{generation}");
    let id = saved
        .ids
        .get(saved.active)
        .filter(|_| !screens[saved.active].ended)
        .map(|id| id.as_str().to_owned());
    let snapshot = ClientShellSnapshot {
        boot_id: boot.clone(),
        revision,
        config_diagnostic: None,
        product_announcement: None,
        update_available: None,
        update_install_command: String::new(),
        server_keybindings_toml: None,
        latest_release_notes_available: false,
        integration_updates_available: false,
        worktree_directory: String::new(),
        release_notes: None,
        focused_workspace_id: Some("herdr-2".into()),
        focused_tab_id: Some("core".into()),
        focused_pane_id: id,
        tab_bar_right: vec![],
        tab_bar_right_separator: String::new(),
        agent_view_label: None,
        agent_order: vec![],
        workspaces: vec![],
        tabs: vec![],
        panes: screens
            .iter()
            .zip(&saved.ids)
            .enumerate()
            .map(|(index, (screen, pane))| ClientShellPane {
                pane_id: pane.as_str().to_owned(),
                workspace_id: "herdr-2".into(),
                tab_id: "core".into(),
                label: screen.title.clone(),
                cwd: None,
                foreground_cwd: None,
                focused: index == saved.active,
                right_click_passthrough: false,
            })
            .collect(),
        agents: vec![],
        commands: vec![],
    };
    let panes = rects
        .iter()
        .zip(&saved.ids)
        .enumerate()
        .filter(|(index, _)| !single || *index == saved.active)
        .map(|(index, (rect, pane_id))| {
            let screen = screens.get(index);
            let rect = SurfaceRect {
                x: rect.x,
                y: rect.y,
                width: rect.cols,
                height: rect.rows,
            };
            PaneSurfacePane {
                pane_id: pane_id.as_str().to_owned(),
                content_revision: revision,
                rect,
                inner_rect: rect,
                // No daemon-drawn bar column: the thumb paints in the pane's
                // last column (`Scrollbar::for_pane`), over its cells.
                scrollbar_rect: None,
                scroll: screen.and_then(|screen| screen.scroll).map(|position| {
                    PaneSurfaceScrollMetrics {
                        offset_from_bottom: position.offset as u64,
                        max_offset_from_bottom: position.history as u64,
                        viewport_rows: u64::from(position.rows),
                    }
                }),
                focused: index == saved.active,
                mouse_reporting: screen.is_some_and(|screen| screen.mouse_reporting),
                sgr_pixel_mouse: false,
                alternate_screen_active: false,
                pixel_width: 0,
                pixel_height: 0,
            }
        })
        .collect();
    let surface = PaneSurfaceFrame {
        boot_id: boot,
        projection_revision: revision,
        surface_revision: revision,
        frame,
        panes,
        splits: vec![],
        popup: None,
        graphics: Default::default(),
    };
    let mut live = Live::default();
    // The session's link, not its panes: a pane whose program ended keeps
    // its final screen and the session stays connected (input to that pane
    // is refused above, through `focused_pane_id`).
    live.connected = screens.iter().any(|s| s.frame.is_some() || s.ended);
    live.input_epoch = epoch;
    live.snapshot = Some(Arc::new(snapshot));
    live.surface = Some(Arc::new(surface));
    live.keyboard_report_all = screens
        .get(saved.active)
        .is_some_and(|s| s.keys.every_key || s.keys.releases);
    live.status = format!(
        "Herdr 2 · {} pane(s) · active {} · {}",
        screens.len(),
        saved.active + 1,
        if single && screens.len() > 1 {
            "One pane"
        } else if saved.vertical {
            "Stacked"
        } else {
            "Side by side"
        }
    );
    let ended: Vec<_> = screens
        .iter()
        .enumerate()
        .filter(|(_, s)| s.ended)
        .map(|(index, _)| (index + 1).to_string())
        .collect();
    if !ended.is_empty() {
        live.status
            .push_str(&format!(" · ended: {}", ended.join(", ")));
    }
    live.dirty = true;
    live
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;

/// Test-only record of what the worker was told to send each terminal,
/// keyed by the whole size so a test finds its own entries. It records inside
/// `pane_sizes`, which both `pane_size` and the hidden-size worker patch use.
#[cfg(test)]
pub(super) mod probe {
    use sheprd_backend::Size;
    use std::sync::{Mutex, PoisonError};

    type Call = (Size, usize, usize, Vec<Option<Size>>);
    static CALLS: Mutex<Vec<Call>> = Mutex::new(Vec::new());

    pub(super) fn record(whole: Size, count: usize, active: usize, sizes: &[Option<Size>]) {
        CALLS.lock().unwrap_or_else(PoisonError::into_inner).push((
            whole,
            count,
            active,
            sizes.to_vec(),
        ));
    }

    /// Every `pane_sizes` call for `whole` and `count`: the active pane and
    /// what each terminal is sent (`None` keeps its size), oldest first.
    pub(crate) fn calls_for(whole: Size, count: usize) -> Vec<(usize, Vec<Option<Size>>)> {
        CALLS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|(w, c, _, _)| *w == whole && *c == count)
            .map(|(_, _, active, sizes)| (*active, sizes.clone()))
            .collect()
    }

    /// The size the drawn pane was sent in each call.
    pub(crate) fn sent_for(whole: Size, count: usize) -> Vec<Size> {
        calls_for(whole, count)
            .into_iter()
            .filter_map(|(_, sizes)| sizes.into_iter().flatten().next())
            .collect()
    }
}
