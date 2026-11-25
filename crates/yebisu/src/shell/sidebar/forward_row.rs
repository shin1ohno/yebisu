//! A port forward's row under its host (issue 140, board 43): `⇄`, the
//! port and its state, an ended forward's reason on a second line. A tap
//! opens a listening forward's page in Safari, or the forward sheet for its
//! host; a long press offers Open and Stop (Dismiss once ended). The rows
//! stay when their host is collapsed, so a running forward never hides.
use super::long_press::{AGENT_MENU_WIDTH, PressedRow, long_press_layer};
use crate::{
    app::Smoke,
    forwards::State,
    shell::{
        forward_sheet,
        sidebar_list::ListForward,
        style::{color, metric, text},
    },
};
use gpui::{
    AnyElement, Context, Pixels, Point, Role, anchored, deferred, div, prelude::*, px, rgb,
};

/// The host long-press menu's forwards (board 41): a 28 pt heading and a
/// 44 pt row per forward, 12 pt in on the left and 8 on the right; buttons
/// 32 high, `#3b3944`, 13 pt.
const MENU_HEADING_HEIGHT: f32 = 28.;
const MENU_ROW_HEIGHT: f32 = 44.;
const MENU_ROW_LEFT: f32 = 12.;
const MENU_ROW_RIGHT: f32 = 8.;
const MENU_NUMBER_WIDTH: f32 = 40.;
const MENU_BUTTON_HEIGHT: f32 = 32.;
const MENU_BUTTON_FILL: u32 = 0x3b3944;
/// The rule above the forwards: 1 pt, 4 pt above and below.
const MENU_RULE_MARGIN: f32 = 4.;

/// Board 43: at least 44 high, 13 pt on an 18 pt line, 6/12/6/40 padding.
pub(crate) const MIN_HEIGHT: f32 = 44.;
pub(crate) const LINE_HEIGHT: f32 = 18.;
pub(crate) const PADDING_VERTICAL: f32 = 6.;
pub(crate) const PADDING_RIGHT: f32 = 12.;
pub(crate) const PADDING_LEFT: f32 = 40.;

impl Smoke {
    /// `row`; `first` marks it for QA (`qa.forward.row`).
    pub(crate) fn forward_row(
        &self,
        row: &ListForward,
        first: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (host, remote, state) = (row.host.clone(), row.remote, row.state);
        let label = self
            .forwards
            .get(&host, remote)
            .map(|forward| forward.label())
            .unwrap_or_default();
        let name = self.setup.host_name(&host).display().to_owned();
        let id = format!("sidebar-forward-{host}-{remote}");
        let selector = id.clone();
        let tap_host = host.clone();
        let mut element = div()
            .id(gpui::ElementId::Name(id.into()))
            .debug_selector(move || selector.clone())
            .role(Role::Button)
            .aria_label(label)
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .justify_center()
            .min_h(px(MIN_HEIGHT))
            .pt(px(PADDING_VERTICAL))
            .pb(px(PADDING_VERTICAL))
            .pr(px(PADDING_RIGHT))
            .pl(px(PADDING_LEFT))
            .text_size(px(text::BASE))
            .line_height(px(LINE_HEIGHT))
            .child(
                div()
                    .flex()
                    .gap(px(metric::TIGHT_GAP))
                    .child(
                        div()
                            .text_color(rgb(forward_sheet::state_color(state)))
                            .child("⇄"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(matches!(state, State::Ended(_)), |line| {
                                line.text_color(rgb(color::RED))
                            })
                            .child(sidebar_line(remote, state)),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.tap_forward_row(&tap_host, remote, window, cx);
            }))
            .child(long_press_layer(
                PressedRow::Forward {
                    host: host.clone(),
                    remote,
                },
                cx,
            ));
        if let State::Ended(reason) = state {
            element = element.child(
                div()
                    .pl(px(metric::INSET))
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(reason.text(&name, remote)),
            );
        }
        if first {
            element = element.children(crate::qa_ax::mark(
                crate::qa_ax::ids::FORWARD_ROW,
                "Forwarded port",
            ));
        }
        element.into_any_element()
    }
}

impl Smoke {
    /// A forward row's long-press menu: Open while it listens, and Stop,
    /// or Dismiss once it ended.
    pub(super) fn render_forward_menu(
        &mut self,
        host: &str,
        remote: u16,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.forwards.get(host, remote)?.state;
        let mut list = div()
            .id("sidebar-forward-menu")
            .w(px(AGENT_MENU_WIDTH))
            .py(px(metric::MENU_PADDING))
            .rounded(px(crate::shell::style::radius::POPOVER))
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
            .shadow(vec![crate::shell::style::shadow::POPOVER.box_shadow()])
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.shell.sidebar.long_press = None;
                cx.notify();
            }));
        let item = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .flex()
                .items_center()
                .h(px(metric::MENU_ITEM))
                .px(px(metric::SIDE))
                .child(label)
        };
        if matches!(state, State::Listening { .. }) {
            let open = host.to_owned();
            list = list.child(item("sidebar-forward-open", "Open").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.shell.sidebar.long_press = None;
                    this.open_forward(&open, remote, cx);
                },
            )));
        }
        let stop = host.to_owned();
        let label = if matches!(state, State::Ended(_)) {
            "Dismiss"
        } else {
            "Stop"
        };
        list = list.child(item("sidebar-forward-stop", label).on_click(cx.listener(
            move |this, _, _, cx| {
                this.shell.sidebar.long_press = None;
                this.stop_forward(&stop, remote, cx);
            },
        )));
        Some(
            deferred(anchored().position(at).snap_to_window().child(list))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// The host long-press menu's "Forwarded ports" and a row per forward
    /// of `host`, or `None` without any.
    pub(super) fn host_menu_forwards(
        &self,
        host: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let forwards: Vec<_> = self
            .forwards
            .for_host(host)
            .map(|forward| (forward.remote.get(), forward.state))
            .collect();
        if forwards.is_empty() {
            return None;
        }
        let name = self.setup.host_name(host).display().to_owned();
        let mut section = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .my(px(MENU_RULE_MARGIN))
                    .h(px(1.))
                    .bg(rgb(color::SEPARATOR)),
            )
            .child(
                div()
                    .h(px(MENU_HEADING_HEIGHT))
                    .px(px(MENU_ROW_LEFT))
                    .flex()
                    .items_center()
                    .text_size(px(text::SECONDARY))
                    .font_weight(crate::shell::style::weight::BOLD)
                    .text_color(rgb(color::MUTED))
                    .child(forward_sheet::LIST_HEADING),
            );
        for (remote, state) in forwards {
            section = section.child(self.host_menu_forward(host, &name, remote, state, cx));
        }
        Some(section.into_any_element())
    }

    fn host_menu_forward(
        &self,
        host: &str,
        name: &str,
        remote: u16,
        state: State,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = |id: String, label: &'static str| {
            div()
                .id(gpui::ElementId::Name(id.into()))
                .role(Role::Button)
                .aria_label(label)
                .h(px(MENU_BUTTON_HEIGHT))
                .px(px(metric::INSET))
                .flex()
                .items_center()
                .rounded(px(crate::shell::style::radius::CONTROL))
                .bg(rgb(MENU_BUTTON_FILL))
                .text_size(px(text::BASE))
                .child(label)
        };
        let mut buttons = div().flex().items_center().gap(px(metric::INLINE_GAP));
        if matches!(state, State::Ended(_)) {
            buttons = buttons.child(div().text_color(rgb(color::RED)).child("ended"));
        }
        if matches!(state, State::Listening { .. }) {
            let open = host.to_owned();
            buttons = buttons.child(
                button(format!("host-menu-forward-open-{remote}"), "Open").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.shell.sidebar.long_press = None;
                        this.open_forward(&open, remote, cx);
                    },
                )),
            );
        }
        let stop = host.to_owned();
        let label = if matches!(state, State::Ended(_)) {
            "Dismiss"
        } else {
            "Stop"
        };
        buttons = buttons.child(
            button(format!("host-menu-forward-stop-{remote}"), label).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.stop_forward(&stop, remote, cx);
                },
            )),
        );
        let state_text = match state {
            State::Ended(_) => String::new(),
            _ => state.row_text(),
        };
        let mut row = div()
            .min_h(px(MENU_ROW_HEIGHT))
            .pl(px(MENU_ROW_LEFT))
            .pr(px(MENU_ROW_RIGHT))
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(metric::INLINE_GAP))
                    .child(
                        div()
                            .w(px(MENU_NUMBER_WIDTH))
                            .font_weight(crate::shell::style::weight::BOLD)
                            .child(remote.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(if matches!(state, State::Listening { .. }) {
                                color::ACCENT
                            } else {
                                color::MUTED
                            }))
                            .child(state_text),
                    )
                    .child(buttons),
            );
        if let State::Ended(reason) = state {
            row = row.child(
                div()
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(reason.text(name, remote)),
            );
        }
        row.into_any_element()
    }
}

/// The row's text after `⇄`: `3000 → 127.0.0.1:3000`, `5173 · connecting…`.
pub(crate) fn sidebar_line(remote: u16, state: State) -> String {
    state
        .sidebar_text(remote)
        .trim_start_matches('⇄')
        .trim_start()
        .to_owned()
}
