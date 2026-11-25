//! Issue #137 PR 5 (C2): a "Jump to bottom" button at the lower right of each
//! pane that shows its scrollback. A tap returns the pane to its bottom, and
//! the button goes with the next surface at offset 0. The design is written
//! in the issue (there is no canvas file): 32 pt high, 16 pt from the pane's
//! right edge and 12 pt from its bottom.
use crate::{app::Smoke, shell::style::color};
use gpui::{
    AnyElement, Bounds, Context, PathBuilder, Pixels, Role, canvas, div, point, prelude::*, px,
    rgb, size,
};

/// The button's text and accessibility name. Shell labels are constants.
pub(crate) const JUMP_TO_BOTTOM: &str = "Jump to bottom";
const RIGHT: f32 = 16.;
const BOTTOM: f32 = 12.;
const HEIGHT: f32 = 32.;
const PADDING_X: f32 = 12.;
const RADIUS: f32 = 16.;
const ARROW: f32 = 14.;
const ARROW_STROKE: f32 = 1.5;
const GAP: f32 = 6.;
const TEXT: f32 = 13.;

impl Smoke {
    /// The panes that show their scrollback, each with the body it is drawn
    /// in, in the order the surface lists them.
    pub(crate) fn scrolled_back_panes(&self) -> Vec<(String, Bounds<Pixels>)> {
        let Some((_, surface)) = self.terminal.coherent_frame() else {
            return Vec::new();
        };
        let scrolled = |pane: &sheprd_client::protocol::PaneSurfacePane| {
            pane.scroll
                .is_some_and(|scroll| scroll.offset_from_bottom > 0)
        };
        let geometry = self.terminal.geometry;
        let framed = geometry.framed_panes(&surface);
        if !framed.is_empty() {
            return framed
                .iter()
                .filter(|placement| scrolled(placement.pane))
                .map(|placement| (placement.pane.pane_id.clone(), placement.body))
                .collect();
        }
        // One pane: drawn in the grid at its own cells.
        let (width, height) = (geometry.cell_width, geometry.cell_height);
        surface
            .panes
            .iter()
            .filter(|pane| scrolled(pane))
            .map(|pane| {
                let rect = pane.inner_rect;
                let body = Bounds::new(
                    geometry.grid.origin
                        + point(
                            px(f32::from(rect.x) * width),
                            px(f32::from(rect.y) * height),
                        ),
                    size(
                        px(f32::from(rect.width) * width),
                        px(f32::from(rect.height) * height),
                    ),
                );
                (pane.pane_id.clone(), body)
            })
            .collect()
    }

    /// The buttons, placed inside the terminal's input element, whose bounds
    /// are the terminal canvas's. Front layers, so a press or a scroll on one
    /// never reaches the terminal under it.
    pub(crate) fn jump_to_bottom_buttons(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let Some(canvas_bounds) = self.terminal.canvas_bounds else {
            return Vec::new();
        };
        self.scrolled_back_panes()
            .into_iter()
            .enumerate()
            .map(|(index, (pane, body))| {
                let button = div()
                    .id(("terminal-jump-bottom", index))
                    .debug_selector(move || format!("terminal-jump-bottom-{index}"))
                    .role(Role::Button)
                    .aria_label(JUMP_TO_BOTTOM)
                    .h(px(HEIGHT))
                    .px(px(PADDING_X))
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .rounded(px(RADIUS))
                    .bg(rgb(color::RAISED))
                    .border_1()
                    .border_color(rgb(color::SEPARATOR))
                    .text_size(px(TEXT))
                    .text_color(rgb(color::TEXT))
                    .cursor_pointer()
                    .child(down_arrow())
                    .child(JUMP_TO_BOTTOM)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.scroll_to_bottom(&pane);
                        cx.notify();
                    }))
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::TERMINAL_JUMP_BOTTOM,
                        JUMP_TO_BOTTOM,
                    ));
                div()
                    .absolute()
                    .right(canvas_bounds.right() - body.right() + px(RIGHT))
                    .bottom(canvas_bounds.bottom() - body.bottom() + px(BOTTOM))
                    .child(crate::front_layer::front_layer(
                        button.into_any_element(),
                        self.front_layers.clone(),
                    ))
                    .into_any_element()
            })
            .collect()
    }
}

/// A 14 pt down arrow: a stem and its head, 1.5 pt strokes.
fn down_arrow() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let mut path = PathBuilder::stroke(px(ARROW_STROKE));
            let center = ARROW / 2.;
            path.move_to(bounds.origin + point(px(center), px(2.)));
            path.line_to(bounds.origin + point(px(center), px(ARROW - 2.)));
            path.move_to(bounds.origin + point(px(3.), px(ARROW - 6.)));
            path.line_to(bounds.origin + point(px(center), px(ARROW - 2.)));
            path.line_to(bounds.origin + point(px(ARROW - 3.), px(ARROW - 6.)));
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color::MUTED));
            }
        },
    )
    .size(px(ARROW))
}
