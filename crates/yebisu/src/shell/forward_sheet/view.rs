//! What the forward sheet draws (board 42): header, intro, the port field,
//! the host's listening ports as chips, the input error, the host's
//! forwards, and the footer's Forward.
use super::{
    CHIP_HEIGHT, CHIP_HIT_HEIGHT, CLOSE, CLOSE_WIDTH, FOOTER_HEIGHT, HEADER_HEIGHT, LIST_HEADING,
    PORT_LABEL, ROW_MIN_HEIGHT, SUBMIT, TITLE, chip_label, chips_heading, intro, sheet_top,
    sheet_width,
};
use crate::{
    app::Smoke,
    forwards::State,
    qa_ax::ids,
    shell::style::{color, metric, radius, shadow, text, tint, weight},
};
use gpui::{AnyElement, Context, MouseButton, Rgba, Window, div, prelude::*, px, rgb};

/// A chip's padding and corner (board 42).
const CHIP_PADDING: f32 = 12.;
const CHIP_RADIUS: f32 = 16.;
/// A row's button: 32 high, `#3b3944`, 13 pt.
const ROW_BUTTON_HEIGHT: f32 = 32.;
const ROW_BUTTON_FILL: u32 = 0x3b3944;
/// An ended row's reason sits under the port, 18 pt in.
const REASON_INSET: f32 = 18.;
/// Forward: 44 high, 20 pt padding, accent fill, `#101419` text.
const SUBMIT_HEIGHT: f32 = 44.;
const SUBMIT_PADDING: f32 = 20.;

impl Smoke {
    /// The open sheet over everything, or `None` while it is closed.
    pub(crate) fn render_forward_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let host = self.shell.forward_sheet.host.clone()?;
        let field = self.shell.forward_sheet.field.clone()?;
        let available = window.fully_visible_bounds();
        let viewport = window.viewport_size();
        let keyboard_up = available.size.height + px(metric::TOUCH) < viewport.height;
        let compact = crate::layout::compact_window(f32::from(viewport.width));
        let width = sheet_width(f32::from(available.size.width));
        let top = sheet_top(compact, keyboard_up);
        let name = self.setup.host_name(&host).display().to_owned();
        let panel = div()
            .id("forward-sheet")
            .debug_selector(|| "forward-sheet".into())
            .absolute()
            .left(available.origin.x + px((f32::from(available.size.width) - width) / 2.))
            .top(available.origin.y + px(top))
            .w(px(width))
            .max_h(px(
                (f32::from(available.size.height) - top - metric::SIDE).max(0.)
            ))
            .flex()
            .flex_col()
            .bg(rgb(color::SURFACE))
            .text_color(rgb(color::TEXT))
            .border_1()
            .border_color(rgb(color::RAISED))
            .rounded(px(radius::SHEET))
            .shadow(vec![shadow::SHEET.box_shadow()])
            .overflow_hidden()
            .child(self.forward_sheet_header(cx))
            .child(
                div()
                    .id("forward-sheet-body")
                    .flex()
                    .flex_col()
                    .gap(px(metric::GAP))
                    .p(px(metric::SIDE))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_color(rgb(color::MUTED))
                            .line_height(gpui::relative(1.5))
                            .child(intro(&name)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(metric::TIGHT_GAP))
                            .child(div().font_weight(weight::SEMIBOLD).child(PORT_LABEL))
                            .child(
                                div()
                                    .relative()
                                    .w(px(super::field::WIDTH))
                                    .child(field)
                                    .children(crate::qa_ax::mark(ids::FORWARD_FIELD, PORT_LABEL)),
                            ),
                    )
                    .children(self.forward_chips(&name, cx))
                    .children(self.forward_error())
                    .children(self.forward_rows(&host, cx)),
            )
            .child(self.forward_sheet_footer(cx));
        Some(
            div()
                .id("forward-overlay")
                .debug_selector(|| "forward-overlay".into())
                .absolute()
                .inset_0()
                .occlude()
                .bg::<Rgba>(tint::SHEET_SCRIM.into())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(panel)
                .into_any_element(),
        )
    }

    fn forward_sheet_header(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .h(px(HEADER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(metric::SMALL_INSET))
            .border_b_1()
            .border_color(rgb(color::RAISED))
            .child(div().w(px(CLOSE_WIDTH)))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_size(px(text::SHEET_TITLE))
                    .font_weight(weight::SEMIBOLD)
                    .child(TITLE),
            )
            .child(
                div()
                    .id("forward-close")
                    .relative()
                    .w(px(CLOSE_WIDTH))
                    .h(px(metric::TOUCH))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(color::ACCENT))
                    .child(CLOSE)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.close_forward_sheet(window, cx)),
                    )
                    .children(crate::qa_ax::mark(ids::FORWARD_CLOSE, CLOSE)),
            )
            .into_any_element()
    }

    /// The chips, or `None` when the host gave no listening ports (or has
    /// no tool to list them).
    fn forward_chips(&self, name: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ports = self.shell.forward_sheet.chips.as_ref()?;
        if ports.is_empty() {
            return None;
        }
        let selected = self.shell.forward_sheet.selected_chip;
        let mut row = div().flex().flex_wrap().gap_x(px(metric::INLINE_GAP));
        for (index, &port) in ports.iter().enumerate() {
            let label = chip_label(port);
            let chosen = selected == Some(port);
            let chip = div()
                .h(px(CHIP_HEIGHT))
                .px(px(CHIP_PADDING))
                .flex()
                .items_center()
                .rounded(px(CHIP_RADIUS))
                .border_1()
                .when(chosen, |chip| {
                    chip.bg::<Rgba>(tint::SELECTED.into())
                        .border_color(rgb(color::ACCENT))
                })
                .when(!chosen, |chip| {
                    chip.bg(rgb(color::RAISED)).border_color(rgb(color::RAISED))
                })
                .child(label.clone());
            row = row.child(
                div()
                    .id(gpui::ElementId::Name(format!("forward-chip-{port}").into()))
                    .relative()
                    .h(px(CHIP_HIT_HEIGHT))
                    .flex()
                    .items_center()
                    .child(chip)
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_forward_chip(port, cx)))
                    .when(index == 0, |hit| {
                        hit.children(crate::qa_ax::mark(ids::FORWARD_CHIP, "Listening port"))
                    }),
            );
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(metric::TIGHT_GAP))
                .child(
                    div()
                        .text_size(px(text::SECONDARY))
                        .font_weight(weight::BOLD)
                        .text_color(rgb(color::MUTED))
                        .child(chips_heading(name)),
                )
                .child(row)
                .into_any_element(),
        )
    }

    fn forward_error(&self) -> Option<AnyElement> {
        let error = self.shell.forward_sheet.error?;
        Some(
            div()
                .id("forward-error")
                .debug_selector(|| "forward-error".into())
                .text_color(rgb(color::RED))
                .child(error.to_string())
                .into_any_element(),
        )
    }

    /// `host`'s forwards with their buttons, under "Forwarded ports".
    fn forward_rows(&self, host: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let rows: Vec<_> = self
            .forwards
            .for_host(host)
            .map(|forward| (forward.remote.get(), forward.state, forward.label()))
            .collect();
        if rows.is_empty() {
            return None;
        }
        let name = self.setup.host_name(host).display().to_owned();
        let mut list = div().flex().flex_col().child(
            div()
                .text_size(px(text::SECONDARY))
                .font_weight(weight::BOLD)
                .text_color(rgb(color::MUTED))
                .child(LIST_HEADING),
        );
        for (remote, state, label) in rows {
            list = list.child(forward_row(host, &name, remote, state, label, cx));
        }
        Some(list.into_any_element())
    }

    fn forward_sheet_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .h(px(FOOTER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_end()
            .px(px(metric::SIDE))
            .border_t_1()
            .border_color(rgb(color::RAISED))
            .child(
                div()
                    .id("forward-submit")
                    .relative()
                    .h(px(SUBMIT_HEIGHT))
                    .px(px(SUBMIT_PADDING))
                    .flex()
                    .items_center()
                    .rounded(px(radius::CONTROL))
                    .bg(rgb(color::ACCENT))
                    .text_color(rgb(color::BACKGROUND))
                    .font_weight(weight::SEMIBOLD)
                    .child(SUBMIT)
                    .on_click(cx.listener(|this, _, window, cx| {
                        let _ = this.submit_forward(window, cx);
                    }))
                    .children(crate::qa_ax::mark(ids::FORWARD_SUBMIT, SUBMIT)),
            )
            .into_any_element()
    }
}

/// The colour of a state's `⇄`: listening success, connecting warning,
/// paused secondary, ended danger. The state is written out too.
pub(crate) fn state_color(state: State) -> u32 {
    match state {
        State::Listening { .. } => color::GREEN,
        State::Connecting => color::YELLOW,
        State::Paused => color::MUTED,
        State::Ended(_) => color::RED,
    }
}

/// One forward in the sheet: `⇄`, the port, its state, its buttons, and an
/// ended forward's reason on a second line.
fn forward_row(
    host: &str,
    name: &str,
    remote: u16,
    state: State,
    label: String,
    cx: &mut Context<Smoke>,
) -> AnyElement {
    let mut buttons = div().flex().gap(px(metric::INLINE_GAP));
    if matches!(state, State::Listening { .. }) {
        let host = host.to_owned();
        buttons = buttons.child(row_button(
            format!("forward-open-{remote}"),
            "Open",
            cx.listener(move |this, _, _, cx| {
                this.open_forward(&host, remote, cx);
            }),
        ));
    }
    let action = if matches!(state, State::Ended(_)) {
        "Dismiss"
    } else {
        "Stop"
    };
    let stop_host = host.to_owned();
    buttons = buttons.child(row_button(
        format!("forward-stop-{remote}"),
        action,
        cx.listener(move |this, _, _, cx| this.stop_forward(&stop_host, remote, cx)),
    ));
    let line = div()
        .flex()
        .items_center()
        .gap(px(metric::INLINE_GAP))
        .child(div().text_color(rgb(state_color(state))).child("⇄"))
        .child(div().font_weight(weight::BOLD).child(remote.to_string()))
        .child(
            div()
                .flex_1()
                .text_color(rgb(match state {
                    State::Listening { .. } => color::ACCENT,
                    State::Ended(_) => color::RED,
                    _ => color::MUTED,
                }))
                .child(state.row_text()),
        )
        .child(buttons);
    let mut row = div()
        .id(gpui::ElementId::Name(
            format!("forward-row-{remote}").into(),
        ))
        .min_h(px(ROW_MIN_HEIGHT))
        .flex()
        .flex_col()
        .justify_center()
        .aria_label(label)
        .child(line);
    if let State::Ended(reason) = state {
        row = row.child(
            div()
                .pl(px(REASON_INSET))
                .text_size(px(text::SECONDARY))
                .text_color(rgb(color::MUTED))
                .child(reason.text(name, remote)),
        );
    }
    row.into_any_element()
}

fn row_button(
    id: String,
    label: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(gpui::ElementId::Name(id.into()))
        .h(px(ROW_BUTTON_HEIGHT))
        .px(px(metric::INSET))
        .flex()
        .items_center()
        .rounded(px(radius::CONTROL))
        .bg(rgb(ROW_BUTTON_FILL))
        .text_size(px(text::BASE))
        .child(label)
        .on_click(on_click)
        .into_any_element()
}
