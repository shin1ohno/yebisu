use super::{TerminalView, pane_names};
use crate::{
    input_geometry::{InputGeometry, PanePlacement},
    live,
    terminal_painter::{Ground, Highlight, ImageTarget, PlacedImages},
};
use gpui::{App, Bounds, ContentMask, Font, Pixels, Size, Window};
use sheprd_client::{
    ConnectOptions,
    protocol::{ClientShellSnapshot, PaneSurfaceFrame},
};
use std::sync::Arc;

/// Reclaim progress for one view and requested size. A matching frame rearms it.
pub(crate) struct Reclaim {
    generation: u64,
    boot: String,
    tab: String,
    options: ConnectOptions,
    progress: ReclaimProgress,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReclaimProgress {
    AwaitingFrame(u64),
    ResizeQueued,
    FocusEdgeStarted,
}

impl TerminalView {
    pub(super) fn update_surface_focus(
        &mut self,
        active: bool,
        resized: bool,
        options: ConnectOptions,
        snapshot: &ClientShellSnapshot,
        surface: &PaneSurfaceFrame,
    ) {
        if self.core_mode {
            return;
        }
        let boot = &snapshot.boot_id;
        let generation = self.link.generation;
        let Some(tab) = snapshot.focused_tab_id.as_deref() else {
            self.report_surface_focus(active);
            return;
        };
        // The outer frame can keep the client's requested size while Herdr
        // crops the shared PTY into it. Scroll metrics expose the runtime rows.
        let mismatch = surface.frame.width != options.surface_size.cols
            || surface.frame.height != options.surface_size.rows
            || surface.panes.iter().any(|pane| {
                pane.scroll
                    .is_some_and(|scroll| scroll.viewport_rows != u64::from(pane.inner_rect.height))
            });
        let claim = |progress| Reclaim {
            generation,
            boot: boot.clone(),
            tab: tab.to_owned(),
            options,
            progress,
        };
        if !mismatch {
            self.surface_reclaim = None;
        } else if resized {
            // The frame drawn while writing a new local size predates that resize.
            self.surface_reclaim = Some(claim(ReclaimProgress::AwaitingFrame(
                surface.surface_revision,
            )));
        }
        let focused = self
            .surface_focus
            .as_ref()
            .is_some_and(|(link, id, focused)| *link == generation && id == boot && *focused);
        if !active || !focused {
            self.report_surface_focus(active);
            return;
        }
        if !mismatch || self.reconnect.suspended || self.navigating.is_some() {
            return;
        }
        let progress = self
            .surface_reclaim
            .as_ref()
            .filter(|claim| {
                claim.generation == generation
                    && &claim.boot == boot
                    && claim.tab == tab
                    && claim.options == options
            })
            .map(|claim| claim.progress);
        if progress == Some(ReclaimProgress::FocusEdgeStarted)
            || progress == Some(ReclaimProgress::AwaitingFrame(surface.surface_revision))
        {
            return;
        }
        let Some(handle) = &self.link.handle else {
            return;
        };
        // Herdr 0.9.x ignores a repeated Focus(true). Queue the measured size
        // first, then make the focus edge that claims the shared PTY.
        if progress != Some(ReclaimProgress::ResizeQueued) {
            if handle.resize(boot, options).is_err() {
                return;
            }
            self.surface_reclaim = Some(claim(ReclaimProgress::ResizeQueued));
        }
        if handle.set_focus(boot, false).is_ok() {
            self.surface_focus = Some((generation, boot.clone(), false));
            self.surface_reclaim = Some(claim(ReclaimProgress::FocusEdgeStarted));
            // If the queue fills before this edge, retry only Focus(true) on
            // the next paint, using the size that was already queued.
            self.report_surface_focus(true);
        }
    }

    pub(super) fn report_surface_focus(&mut self, active: bool) {
        if self.core_mode || (active && (self.reconnect.suspended || self.navigating.is_some())) {
            return;
        }
        let (Some(handle), Some(snapshot)) = (&self.link.handle, &self.live.snapshot) else {
            return;
        };
        let boot = &snapshot.boot_id;
        let generation = self.link.generation;
        let reported = self
            .surface_focus
            .as_ref()
            .filter(|(link, id, _)| *link == generation && id == boot)
            .map(|(_, _, focused)| *focused);
        if reported.unwrap_or(false) == active {
            return;
        }
        // Herdr applies a resize to the shared PTY only for the tab's geometry
        // controller. Report focus after queuing the measured size so a newly
        // active view claims it before Herdr crops the PTY's bottom rows.
        if handle.set_focus(boot, active).is_ok() {
            self.surface_focus = Some((generation, boot.clone(), active));
        }
    }

    pub(crate) fn column_bounds(&self) -> Bounds<Pixels> {
        let geometry = self.geometry;
        self.canvas_bounds
            .filter(|bounds| {
                InputGeometry::from_canvas(*bounds, geometry.cell_width, geometry.cell_height).grid
                    == geometry.grid
            })
            .unwrap_or(geometry.grid)
    }

    pub(crate) fn coherent_frame(
        &self,
    ) -> Option<(Arc<ClientShellSnapshot>, Arc<PaneSurfaceFrame>)> {
        let snapshot = self.live.snapshot.as_ref()?;
        let surface = self.live.surface.as_ref()?;
        live::coherent(snapshot, surface).then(|| (snapshot.clone(), surface.clone()))
    }

    pub(super) fn viewport_size(&self, panes: &[PanePlacement<'_>]) -> Size<Pixels> {
        match panes {
            [first, second, ..] if self.core_mode => self
                .geometry
                .equal_pane_viewport(panes.len(), first.pane.rect.y != second.pane.rect.y),
            _ => self.geometry.viewport_size(panes),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_surface(
        &mut self,
        (snapshot, surface): (&ClientShellSnapshot, &PaneSurfaceFrame),
        panes: &[PanePlacement<'_>],
        highlights: &[Highlight],
        font: &Font,
        chip: &crate::shell::rename::labels::ChipRename,
        window: &mut Window,
        cx: &mut App,
    ) {
        let geometry = self.geometry;
        let grid = geometry.grid;
        let assets = &self.live.surface_images;
        let placements = &surface.graphics.placements;
        let main_highlights = if surface.popup.is_none() {
            highlights
        } else {
            &[]
        };
        let bounds = self.column_bounds();
        let painter = &mut self.painter;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if panes.is_empty() {
                painter.paint_frame(
                    &surface.frame,
                    grid.origin,
                    Some(grid.size),
                    Ground::Opaque,
                    geometry.cell_width,
                    font,
                    main_highlights,
                    &surface.panes,
                    None,
                    Some(PlacedImages {
                        placements,
                        images: assets,
                        target: ImageTarget::Main,
                    }),
                    window,
                    cx,
                );
                // Issue #143: the single pane has no chip; a rename of it
                // draws one for the field to cover.
                if let Some(id) = snapshot.focused_pane_id.as_deref()
                    && chip.editing(id)
                {
                    let name = pane_names::name(snapshot, id, 0);
                    let at = painter.paint_single_chip(
                        &name,
                        Bounds::new(grid.origin, grid.size),
                        window,
                        cx,
                    );
                    chip.report(id, at, window.content_mask().bounds);
                }
            } else {
                for (index, placement) in panes.iter().copied().enumerate() {
                    let id = &placement.pane.pane_id;
                    let pending = chip.pending_for(id);
                    let look = crate::shell::rename::labels::ChipLook {
                        ink: pending.as_ref().map(|(_, ink)| *ink),
                        hidden: chip.editing(id),
                    };
                    let name = match pending {
                        Some((text, _)) => std::borrow::Cow::Owned(text),
                        None => pane_names::name(snapshot, id, index),
                    };
                    let at = painter.paint_framed_pane(
                        surface,
                        placement,
                        &name,
                        look,
                        geometry.cell_width,
                        font,
                        main_highlights,
                        assets,
                        window,
                        cx,
                    );
                    if look.hidden {
                        chip.report(id, at, window.content_mask().bounds);
                    }
                }
            }
            let area = geometry.area(Some(surface));
            if let Some(popup) = &surface.popup {
                painter.paint_frame(
                    &popup.frame,
                    area.origin,
                    None,
                    Ground::Opaque,
                    geometry.cell_width,
                    font,
                    highlights,
                    &[],
                    None,
                    Some(PlacedImages {
                        placements,
                        images: assets,
                        target: ImageTarget::Popup(&popup.terminal_id),
                    }),
                    window,
                    cx,
                );
            }
            painter.paint_composition(
                &self.input.composition.text,
                geometry.cursor(Some(surface)).origin,
                area,
                font,
                window,
            );
        });
    }
}
