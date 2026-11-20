use super::{TerminalView, pane_names};
use crate::{
    input_geometry::PanePlacement,
    live,
    terminal_painter::{Ground, Highlight, ImageTarget, PlacedImages},
};
use gpui::{App, ContentMask, Font, Pixels, Size, Window};
use sheprd_client::protocol::{ClientShellSnapshot, PaneSurfaceFrame};
use std::sync::Arc;

impl TerminalView {
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
        let grid = geometry.terminal_grid();
        let assets = &self.live.surface_images;
        let placements = &surface.graphics.placements;
        let main_highlights = if surface.popup.is_none() {
            highlights
        } else {
            &[]
        };
        let painter = &mut self.painter;
        window.with_content_mask(
            Some(ContentMask {
                bounds: geometry.grid,
            }),
            |window| {
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
            },
        );
    }
}
