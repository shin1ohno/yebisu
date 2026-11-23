//! The underline that shows, while Command is held, that the web address
//! under the pointer opens (L66, artboard OpenUrl). It is the fork's hovered
//! link line (`TerminalPainter::paint_link`): one point, solid, in each
//! cell's text colour, on every row of the address, where an explicit
//! hyperlink's dashes are drawn at rest, so it replaces them. A finger has no
//! hover, so only a pointer shows it; links that open nothing show none.
use super::{Link, Target, pane_link_at};
use crate::{
    app::Smoke, input_geometry::PanePlacement, terminal::in_rect, terminal_view::TerminalView,
};
use gpui::{Context, ModifiersChangedEvent, MouseMoveEvent, Pixels, Point, Window};
use sheprd_client::protocol::PaneSurfaceFrame;
use std::ops::Range;

/// Where the pointer rests over the terminal, and the rows last underlined.
#[derive(Default)]
pub(crate) struct Hover {
    position: Option<Point<Pixels>>,
    shown: Vec<(u16, Range<u16>)>,
}

impl Hover {
    /// The rows underlined now.
    #[cfg(test)]
    pub(crate) fn underlined(&self) -> &[(u16, Range<u16>)] {
        &self.shown
    }

    /// Takes the pointer to rest at `position` (QA's forced hover).
    #[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
    pub(crate) fn rest_at(&mut self, position: Point<Pixels>) {
        self.position = Some(position);
    }
}

impl TerminalView {
    /// The rows of the web address under the resting pointer while Command
    /// is held; none otherwise.
    pub(crate) fn hovered_web_rows(&self, command: bool) -> Vec<(u16, Range<u16>)> {
        let Some(position) = self.link_hover.position.filter(|_| command) else {
            return Vec::new();
        };
        let Some((_, surface)) = self.coherent_frame() else {
            return Vec::new();
        };
        let Some((column, row)) = self.geometry.cell_at(&surface, position, None) else {
            return Vec::new();
        };
        match pane_link_at(&surface, column, row) {
            Some(
                link @ Link {
                    target: Target::Web(_),
                    ..
                },
            ) => link.rows().collect(),
            _ => Vec::new(),
        }
    }

    /// Underlines the hovered web address, after the frame is painted.
    pub(crate) fn paint_link_hover(
        &self,
        surface: &PaneSurfaceFrame,
        panes: &[PanePlacement<'_>],
        window: &mut Window,
    ) {
        #[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
        let forced = super::qa::forced_hover();
        #[cfg(not(any(test, all(target_os = "ios", target_abi = "sim"))))]
        let forced = false;
        let rows = self.hovered_web_rows(window.modifiers().platform || forced);
        let Some((row, columns)) = rows.first() else {
            return;
        };
        // Framed panes paint the frame from their own origin; one pane, or
        // none, from the grid's.
        let origin = panes
            .iter()
            .find(|placement| in_rect(placement.pane.inner_rect, columns.start, *row))
            .map_or(self.geometry.grid.origin, |placement| {
                placement.frame_origin
            });
        self.painter.paint_link(
            &surface.frame,
            origin,
            self.geometry.cell_width,
            &rows,
            window,
        );
    }
}

impl Smoke {
    /// The pointer moved over the terminal. A drag is no hover.
    pub(crate) fn link_hover_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        self.terminal.link_hover.position =
            event.pressed_button.is_none().then_some(event.position);
        self.refresh_link_hover(event.modifiers.platform, cx);
    }

    /// Command went down or up while the pointer may rest on an address.
    pub(crate) fn link_hover_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        cx: &mut Context<Self>,
    ) {
        self.refresh_link_hover(event.modifiers.platform, cx);
    }

    /// The pointer left the terminal.
    pub(crate) fn link_hover_left(&mut self, cx: &mut Context<Self>) {
        self.terminal.link_hover.position = None;
        self.refresh_link_hover(false, cx);
    }

    /// Repaints only when the underlined rows change.
    fn refresh_link_hover(&mut self, command: bool, cx: &mut Context<Self>) {
        let rows = self.terminal.hovered_web_rows(command);
        if rows != self.terminal.link_hover.shown {
            self.terminal.link_hover.shown = rows;
            cx.notify();
        }
    }
}
