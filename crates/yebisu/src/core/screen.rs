use crate::live::Live;
use sheprd_backend::{FrameData, KeyReporting, Size, SurfaceUpdate};
use sheprd_client::protocol::{CellData, ClientShellSnapshot, PaneSurfaceFrame};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Screen {
    pub frame: Option<FrameData>,
    pub keys: KeyReporting,
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
            _ => {}
        }
    }
}
pub(super) fn pane_size(size: Size, count: usize, vertical: bool) -> crate::Result<Size> {
    let count = count.max(1) as u16;
    Ok(if vertical {
        Size::new(size.cols(), (size.rows() / count).max(1))?
    } else {
        Size::new((size.cols() / count).max(1), size.rows())?
    })
}
pub(super) fn publish(
    screens: &[Screen],
    saved: &super::store::Saved,
    size: Size,
    generation: u64,
    revision: u64,
    epoch: u64,
) -> Live {
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
    for (index, screen) in screens.iter().enumerate() {
        let Some(source) = &screen.frame else {
            continue;
        };
        let (x, y) = if saved.vertical {
            (0, index as u16 * (size.rows() / screens.len() as u16))
        } else {
            (index as u16 * (size.cols() / screens.len() as u16), 0)
        };
        for row in 0..source.height.min(size.rows().saturating_sub(y)) {
            for col in 0..source.width.min(size.cols().saturating_sub(x)) {
                if let Some(cell) = source
                    .cells
                    .get(usize::from(row) * usize::from(source.width) + usize::from(col))
                {
                    frame.cells
                        [usize::from(y + row) * usize::from(size.cols()) + usize::from(x + col)] =
                        cell.clone();
                }
            }
        }
        if index == saved.active {
            frame.cursor = source.cursor.clone().map(|mut cursor| {
                cursor.x += x;
                cursor.y += y;
                cursor
            });
        }
    }
    let boot = format!("core-{generation}");
    let id = saved.ids.get(saved.active).map(|id| id.as_str().to_owned());
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
        panes: vec![],
        agents: vec![],
        commands: vec![],
    };
    let surface = PaneSurfaceFrame {
        boot_id: boot,
        projection_revision: revision,
        surface_revision: revision,
        frame,
        panes: vec![],
        splits: vec![],
        popup: None,
        graphics: Default::default(),
    };
    let mut live = Live::default();
    live.connected = screens.get(saved.active).is_some_and(|s| s.frame.is_some());
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
        if saved.vertical {
            "Stacked"
        } else {
            "Side by side"
        }
    );
    live.dirty = true;
    live
}
