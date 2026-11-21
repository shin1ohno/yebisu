use super::{TerminalView, pane_names};
use crate::{
    input_geometry::{InputGeometry, PanePlacement},
    live,
    terminal_painter::{Ground, Highlight, ImageTarget, PlacedImages},
};
use gpui::{App, Bounds, ContentMask, Font, Pixels, Size, Window};
use sheprd_client::protocol::{ClientShellSnapshot, PaneSurfaceFrame};
use std::sync::Arc;

impl TerminalView {
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

    pub(super) fn coherent_frame(
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

    pub(super) fn paint_surface(
        &mut self,
        (snapshot, surface): (&ClientShellSnapshot, &PaneSurfaceFrame),
        panes: &[PanePlacement<'_>],
        highlights: &[Highlight],
        font: &Font,
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
            } else {
                for (index, placement) in panes.iter().copied().enumerate() {
                    let name = pane_names::name(snapshot, &placement.pane.pane_id, index);
                    painter.paint_framed_pane(
                        surface,
                        placement,
                        &name,
                        geometry.cell_width,
                        font,
                        main_highlights,
                        assets,
                        window,
                        cx,
                    );
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
