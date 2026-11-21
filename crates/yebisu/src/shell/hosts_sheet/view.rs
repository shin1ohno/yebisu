//! Drawing for the first-run frame, the Hosts sheet and the Add host sheet.
//! Colours and sizes follow canvas AddHost.dc.html; no rule lives here.
use super::{DEVELOPER_BUTTONS, developer_items, name_field::NameField};
use crate::{
    app::Smoke,
    layout::SIDEBAR_WIDTH,
    settings::key_ops::State,
    setup::Job,
    shell::{
        add_host::{AddHostStep, SHEET_WIDTH, SheetMode, sheet_height, sheet_width},
        style::{color, metric, radius, shadow, text, tint, weight},
    },
};
use gpui::{
    AnyElement, Context, ElementId, MouseButton, Rgba, Role, SharedString, Stateful, Window,
    canvas, div, prelude::*, px, rgb,
};

// AddHost.dc.html:29–33
const ADD_HOST_TOP: f32 = 96.;
const HEADER_BUTTON_WIDTH: f32 = 88.;
// AddHost.dc.html:29–83, rendered panel from y=96 through y=720.
const REFERENCE_HEIGHT: f32 = 624.;
// AddHost.dc.html:36
const BODY_TOP_PADDING: f32 = 20.;
const BODY_SIDE_PADDING: f32 = 24.;
const BODY_GAP: f32 = 20.;
// AddHost.dc.html:41–45
const BADGE_SIZE: f32 = 24.;
const STEP_INDENT: f32 = 34.;
const KEY_PADDING_Y: f32 = 10.;
const KEY_PADDING_X: f32 = 12.;
// AddHost.dc.html:61–62
pub(super) const HOST_CONTROL_HEIGHT: f32 = 48.;
// AddHost.dc.html:64
const FINGERPRINT_CARD: u32 = 0x22222a;
const FINGERPRINT_SIDE_PADDING: f32 = 14.;
// AddHost.dc.html:75
const UPCOMING_STEP_OPACITY: f32 = 0.6;

const KEY_COPY_LABEL: &str = "Copy authorized_keys line";
const HOST_OPEN_LABEL: &str = "Open";

pub(super) fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    primary: bool,
) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    div()
        .id(ElementId::Name(id.clone()))
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .min_h(px(metric::TOUCH))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .rounded(px(radius::CONTROL))
        .bg(rgb(if primary && enabled {
            color::ACCENT
        } else {
            color::RAISED
        }))
        .text_color(rgb(if primary && enabled {
            color::BACKGROUND
        } else if enabled {
            color::TEXT
        } else {
            color::MUTED
        }))
        .when(!enabled, |button| button.opacity(0.45))
        .child(label)
}

fn header_text_button(id: &'static str, label: &'static str, enabled: bool) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::Button)
        .aria_label(label)
        .when(!enabled, |button| {
            button.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
        .w(px(HEADER_BUTTON_WIDTH))
        .h(px(metric::TOUCH))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(if enabled { color::ACCENT } else { color::MUTED }))
        .child(label)
}

fn step() -> gpui::Div {
    div().flex().flex_col().gap(px(metric::ROW_GAP))
}

fn badge(number: u8, done: bool, active: bool) -> impl IntoElement {
    div()
        .size(px(BADGE_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .font_weight(weight::BOLD)
        .when(done, |d| {
            d.bg(rgb(color::GREEN)).text_color(rgb(color::BACKGROUND))
        })
        .when(!done && active, |d| {
            d.bg(rgb(color::ACCENT)).text_color(rgb(color::BACKGROUND))
        })
        .when(!done && !active, |d| {
            d.border_2()
                .border_color(rgb(color::MUTED))
                .text_color(rgb(color::MUTED))
        })
        .child(number.to_string())
}

fn step_title(number: u8, done: bool, active: bool, title: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(metric::ROW_GAP))
        .child(badge(number, done, active))
        .child(
            div()
                .text_size(px(text::NAME))
                .font_weight(weight::SEMIBOLD)
                .child(title),
        )
}

impl Smoke {
    fn record_setup_observation(&self) {
        self.observation.record(serde_json::json!({
            "kind": "setup", "busy": self.setup.busy, "status": self.setup.status,
            "target": self.setup_input.text, "fingerprints": self.setup.fingerprints,
            "key_ready": self.hosts_key_line().is_some(), "hosts": self.setup.hosts,
        }));
    }

    pub(super) fn draw_first_run(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.terminal.open {
            return None;
        }
        let opened = self
            .shell
            .hosts_sheet
            .auto_open(self.setup.loaded, self.setup.hosts.len());
        if opened && self.shell.hosts_sheet.add.is_some() {
            self.read_device_key(cx);
        }
        self.record_setup_observation();
        let sidebar_width = SIDEBAR_WIDTH.min(f32::from(window.viewport_size().width));
        let hosts = self.setup.hosts.clone();
        let selected = self.setup.target.clone();
        let mut list = div().id("first-run-hosts").flex().flex_col().gap_1().p_2();
        for (index, host) in hosts.into_iter().enumerate() {
            let target = host.clone();
            let label = self
                .setup
                .saved_name(&host)
                .map_or_else(|| host.clone(), |name| format!("{name} · {host}"));
            list = list.child(
                button(
                    format!("first-run-host-{index}"),
                    label,
                    !self.setup.busy,
                    host == selected,
                )
                .justify_start()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.hosts_open_add(SheetMode::Open, &target, cx);
                })),
            );
        }
        let sidebar = div()
            .w(px(sidebar_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(color::SURFACE))
            .border_r_1()
            .border_color(rgb(color::RAISED))
            .child(
                div()
                    .h(px(44.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(color::MUTED))
                            .font_weight(weight::SEMIBOLD)
                            .child("HOSTS"),
                    ),
            )
            .child(
                div()
                    .id("first-run-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        button("first-run-add-host", "Add host", !self.setup.busy, true).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.hosts_open_add(SheetMode::Add, "", cx);
                            }),
                        ),
                    )
                    .child(self.settings_button(cx)),
            );
        Some(
            div()
                .id("first-run")
                .size_full()
                .flex()
                .bg(rgb(color::BACKGROUND))
                .text_color(rgb(color::TEXT))
                .track_focus(&self.focus)
                .child(sidebar)
                .child(div().id("first-run-terminal-column").flex_1().h_full())
                .into_any_element(),
        )
    }

    pub(super) fn draw_hosts_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.shell.hosts_sheet.note_terminal(self.terminal.open);
        if !self.shell.hosts_sheet.visible() {
            return None;
        }
        if self.terminal.open {
            self.record_setup_observation();
        }
        let available = window.fully_visible_bounds();
        // The screenshot fixture captures the whole sheet with the keyboard down.
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        if std::env::var_os("YEBISU_QA_NEARBY_SCREENSHOT").is_some()
            && self.shell.hosts_sheet.add.is_some()
            && available.size.height + px(metric::TOUCH) < window.viewport_size().height
        {
            window.on_next_frame(|window, _| window.dismiss_virtual_keyboard());
        }
        let width = sheet_width(f32::from(available.size.width));
        let available_height = f32::from(available.size.height);
        let margin = crate::layout::OVERLAY_MARGIN / 2.;
        let is_add = self.shell.hosts_sheet.add.is_some();
        let top = if is_add
            && width == SHEET_WIDTH
            && available_height >= ADD_HOST_TOP + REFERENCE_HEIGHT + margin
        {
            ADD_HOST_TOP
        } else {
            margin
        };
        let height = (sheet_height(available_height) - (top - margin)).max(0.);
        let body = if is_add {
            self.add_host_sheet(cx)
        } else {
            self.hosts_list_sheet(cx)
        };
        let overlay = div()
            .id("hosts-overlay")
            .debug_selector(|| "hosts-overlay".into())
            .absolute()
            .inset_0()
            .occlude()
            .bg::<Rgba>(tint::SHEET_SCRIM.into())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("hosts-panel")
                    .debug_selector(|| "hosts-panel".into())
                    .absolute()
                    .left(available.origin.x + px((f32::from(available.size.width) - width) / 2.))
                    .top(available.origin.y + px(top))
                    .w(px(width))
                    .max_h(px(height))
                    .flex()
                    .flex_col()
                    .bg(rgb(color::SURFACE))
                    .text_color(rgb(color::TEXT))
                    .border_1()
                    .border_color(rgb(color::RAISED))
                    .rounded(px(radius::SHEET))
                    .shadow(vec![shadow::SHEET.box_shadow()])
                    .overflow_hidden()
                    .child(body),
            );
        Some(overlay.into_any_element())
    }

    fn sheet_header(&self, title: &'static str, left: AnyElement, right: AnyElement) -> AnyElement {
        div()
            .h(px(60.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_2()
            .border_b_1()
            .border_color(rgb(color::RAISED))
            .child(div().w(px(104.)).flex().justify_start().child(left))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .font_weight(weight::SEMIBOLD)
                    .child(title),
            )
            .child(div().w(px(104.)).flex().justify_end().child(right))
            .into_any_element()
    }

    fn hosts_key_line(&self) -> Option<&str> {
        if self.hosts_open_signing_in() {
            return None;
        }
        self.settings.as_ref().and_then(|host| host.keys.line())
    }

    /// The public line is truncated for display and copied in full.
    fn key_block(&self, line: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let keys = self.settings.as_ref().map(|host| &host.keys);
        let signing_in = self.hosts_open_signing_in();
        let key_ready = line.is_some();
        let copy_enabled = !self.setup.busy && key_ready;
        crate::qa_ax::report("key_ready", || key_ready.into());
        let key_text = if signing_in {
            "Waiting for sign-in to finish…"
        } else {
            line.unwrap_or_else(|| keys.map_or("Device key unavailable.", |keys| keys.label()))
        };
        let key_needs_attention = !key_ready
            && !signing_in
            && !keys.is_some_and(|keys| matches!(keys.state, State::Reading | State::Submitted));
        let provider_mark = self
            .key_qa_label()
            .and_then(|label| crate::qa_ax::mark(crate::qa_ax::ids::KEYS_PROVIDER, label));
        div()
            .flex()
            .flex_col()
            .gap(px(metric::INLINE_GAP))
            .child(
                div()
                    .id("device-key-line")
                    .when(signing_in, |key| {
                        key.debug_selector(|| "device-key-sign-in-progress".into())
                    })
                    .relative()
                    .w_full()
                    .min_w_0()
                    .truncate()
                    .px(px(KEY_PADDING_X))
                    .py(px(KEY_PADDING_Y))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(rgb(color::RAISED))
                    .bg(rgb(color::BACKGROUND))
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .font_family("SauceCodePro Nerd Font Mono")
                    .child(key_text.to_owned())
                    .children(provider_mark),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(KEY_PADDING_X))
                    .child(
                        button("key-copy", KEY_COPY_LABEL, copy_enabled, false)
                            .when(!copy_enabled, |button| {
                                button.a11y_synthetic_children(|builder| {
                                    builder.parent_node().set_disabled()
                                })
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.setup.busy
                                    && this.hosts_key_line().is_some()
                                    && this.copy_device_key(cx)
                                {
                                    this.setup.status = "Copied restricted public key.".into();
                                    this.qa_ax_key_copied();
                                    cx.notify();
                                }
                            }))
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::KEYS_COPY,
                                KEY_COPY_LABEL,
                            )),
                    )
                    .child(
                        div()
                            .text_color(rgb(color::MUTED))
                            .child("Add it to ~/.ssh/authorized_keys on the host."),
                    ),
            )
            .when(key_needs_attention, |block| {
                block.child(
                    div()
                        .id("device-key-guidance")
                        .debug_selector(|| "device-key-guidance".into())
                        .text_color(rgb(color::MUTED))
                        .child("To try again, read the device key in Settings › Hosts and keys."),
                )
            })
            .into_any_element()
    }

    fn hosts_list_sheet(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.setup.busy;
        let mut rows = div().id("hosts-rows").flex().flex_col().gap_2();
        let hosts = self.setup.hosts.clone();
        if hosts.is_empty() {
            rows = rows.child(div().text_color(rgb(color::MUTED)).child("No hosts yet."));
        }
        for (index, host) in hosts.into_iter().enumerate() {
            let display = self.setup.host_name(&host);
            let label = self
                .setup
                .saved_name(&host)
                .map_or_else(|| host.clone(), |_| format!("{display} · {host}"));
            let open = host.clone();
            let edit = host.clone();
            // The saved-host QA fixture uses the same host selection identifier
            // in this sheet as in the sidebar.
            let open_mark = (host == "qa@qa-alpha.invalid")
                .then(|| {
                    crate::qa_ax::mark(crate::qa_ax::ids::SIDEBAR_HOST_QA_ALPHA, HOST_OPEN_LABEL)
                })
                .flatten();
            rows = rows.child(
                div()
                    .id(("hosts-row", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().overflow_hidden().child(label))
                    .child(
                        button(format!("host-open-{index}"), HOST_OPEN_LABEL, !busy, true)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.hosts_open_add(SheetMode::Open, &open, cx);
                            }))
                            .children(open_mark),
                    )
                    .child(
                        button(format!("host-edit-{index}"), "Edit", !busy, false).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.hosts_open_add(SheetMode::Edit, &edit, cx);
                            }),
                        ),
                    ),
            );
        }
        let mut developer = div().flex().flex_col().gap_2();
        for label in developer_items(DEVELOPER_BUTTONS) {
            developer = developer.child(button(label, label, !busy, false).on_click(cx.listener(
                move |this, _, window, cx| {
                    if this.setup.busy {
                        return;
                    }
                    if label == "Developer: simulate save failure" {
                        this.start_job(Job::SaveFailure, cx);
                    } else {
                        this.smoke_mode = true;
                        this.shell.hosts_sheet.close();
                        window.dismiss_virtual_keyboard();
                        cx.notify();
                    }
                },
            )));
        }
        let close = button("hosts-close", "Close", true, false)
            .on_click(cx.listener(|this, _, window, cx| {
                this.hosts_close(cx);
                this.return_keys_to_terminal(window, cx);
            }))
            .into_any_element();
        let add = button("hosts-add", "Add host", !busy, true)
            .on_click(cx.listener(|this, _, _, cx| this.hosts_open_add(SheetMode::Add, "", cx)))
            .into_any_element();
        let header = self.sheet_header("Hosts", close, add);
        div()
            .id("hosts-list-sheet")
            .flex()
            .flex_col()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .id("hosts-list-body")
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(rows)
                    .child(
                        div()
                            .id("setup-status")
                            .text_sm()
                            .text_color(rgb(color::MUTED))
                            .child(self.setup.status.clone()),
                    )
                    .child(developer),
            )
            .into_any_element()
    }

    fn add_host_sheet(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.hosts_sync_field();
        let Some(session) = self.shell.hosts_sheet.add.clone() else {
            return div().into_any_element();
        };
        let flow = &session.flow;
        let key_line = self.hosts_key_line();
        let key_ready = key_line.is_some();
        let key = self.key_block(key_line, cx);
        let scanning = flow.scanning && flow.step == AddHostStep::Address;
        let trusting = flow.scanning && flow.step == AddHostStep::Trust;
        let host_done = flow.step == AddHostStep::SignIn;
        let input_entity = cx.entity();
        let field = div()
            .id("target-field")
            .flex_1()
            .min_w_0()
            .h(px(HOST_CONTROL_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(rgb(color::ACCENT))
            .bg(rgb(color::BACKGROUND))
            .relative()
            .on_click(cx.listener(|this, _, window, cx| {
                this.focus.focus(window, cx);
                window.request_virtual_keyboard();
            }))
            .child(if self.setup_input.text.is_empty() {
                "user@host[:port]".to_owned()
            } else {
                self.setup_input.text.clone()
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let focus = input_entity.read(cx).focus.clone();
                        window.handle_input(
                            &focus,
                            gpui::ElementInputHandler::new(bounds, input_entity),
                            cx,
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let scan_control = if scanning {
            button("host-scan-cancel", "Cancel", true, false)
                .h(px(HOST_CONTROL_HEIGHT))
                .on_click(cx.listener(|this, _, _, cx| this.hosts_cancel_scan(cx)))
        } else {
            button(
                "host-scan",
                "Scan",
                flow.can_scan() && !self.nearby_needs_username(),
                false,
            )
            .h(px(HOST_CONTROL_HEIGHT))
            .aria_label("Scan host key")
            .on_click(cx.listener(|this, _, _, cx| this.hosts_scan(cx)))
        };
        let mut check = div()
            .ml(px(STEP_INDENT))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(color::MUTED)).child("Host"))
            .child(div().flex().gap_2().child(field).child(scan_control));
        if self.nearby_needs_username() {
            check = check.child(
                div()
                    .text_color(rgb(color::MUTED))
                    .child("Add your SSH username before the host."),
            );
        }
        if session.mode != SheetMode::Open {
            let name = self
                .shell
                .hosts_sheet
                .name
                .get_or_insert_with(|| {
                    let saved = self.setup.saved_name(&flow.typed_host).map(str::to_owned);
                    cx.new(|cx| NameField::new(saved, cx))
                })
                .clone();
            name.update(cx, |field, _| field.set_enabled(!self.setup.busy));
            check = check
                .children(self.nearby_list(cx))
                .child(div().text_color(rgb(color::MUTED)).child("Name"))
                .child(name);
        }
        if scanning {
            check = check.child(div().id("scan-progress").child("Scanning…"));
        }
        if let (AddHostStep::Trust, Some(fingerprint)) = (flow.step, &flow.scanned_fingerprint) {
            check = check.child(
                div()
                    .id("fingerprint-box")
                    .bg(rgb(FINGERPRINT_CARD))
                    .px(px(FINGERPRINT_SIDE_PADDING))
                    .py(px(KEY_PADDING_X))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .rounded(px(radius::CARD))
                    .border_1()
                    .border_color(rgb(color::RAISED))
                    .child(
                        div()
                            .text_color(rgb(color::MUTED))
                            .child("Compare this with the host before you trust it:"),
                    )
                    .child(
                        div()
                            .id("host-fingerprints")
                            .font_family("SauceCodePro Nerd Font Mono")
                            .child(fingerprint.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                button("host-reject", "Not this host", !trusting, false)
                                    .bg(gpui::transparent_black())
                                    .text_color(rgb(color::MUTED))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.hosts_reject_key(cx)),
                                    ),
                            )
                            .child(
                                button("host-trust", "Trust key", flow.can_trust(), true)
                                    .aria_label("Trust the compared fingerprint and save")
                                    .font_weight(weight::SEMIBOLD)
                                    .on_click(cx.listener(|this, _, _, cx| this.hosts_trust(cx))),
                            ),
                    ),
            );
        }
        if let Some(notice) = session.notice.as_ref().filter(|_| !host_done) {
            check = check.child(self.notice_block(notice.lines(), "notice-host-key"));
        }
        let mut sign_in = div()
            .ml(px(STEP_INDENT))
            .flex()
            .flex_col()
            .gap(px(metric::INLINE_GAP))
            .child(div().text_color(rgb(color::MUTED)).child(
                "Checks SSH sign-in, then opens Herdr 1. If only Herdr 2 runs there, it opens Herdr 2. If sign-in fails or no Herdr answers, this step shows the reason and Retry.",
            ));
        if session.signing_in {
            sign_in = sign_in.child(div().id("sign-in-progress").child("Signing in…"));
        }
        if let Some(notice) = session.notice.as_ref().filter(|_| host_done) {
            if notice.offers_retry() {
                sign_in = sign_in.child(
                    button("host-auth", "Retry", session.can_sign_in(), true)
                        .on_click(cx.listener(|this, _, _, cx| this.hosts_sign_in(cx))),
                );
            }
            sign_in = sign_in.child(self.notice_block(notice.lines(), "notice-sign-in"));
        }
        let cancel = header_text_button("hosts-add-cancel", "Cancel", true).on_click(cx.listener(
            |this, _, window, cx| {
                this.hosts_close(cx);
                this.return_keys_to_terminal(window, cx);
            },
        ));
        let connect = header_text_button("hosts-add-connect", "Connect", session.can_sign_in())
            .on_click(cx.listener(|this, _, _, cx| this.hosts_sign_in(cx)));
        let header = div()
            .id("add-host-header")
            .debug_selector(|| "add-host-header".into())
            .h(px(metric::SHEET_HEADER))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(metric::INLINE_GAP))
            .border_b_1()
            .border_color(rgb(color::RAISED))
            .child(cancel)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_center()
                    .text_size(px(text::SHEET_TITLE))
                    .font_weight(weight::SEMIBOLD)
                    .child(session.mode.title()),
            )
            .child(connect);
        div()
            .id("add-host-sheet")
            .flex()
            .flex_col()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .id("add-host-body")
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt(px(BODY_TOP_PADDING))
                    .px(px(BODY_SIDE_PADDING))
                    .pb(px(BODY_SIDE_PADDING))
                    .flex()
                    .flex_col()
                    .gap(px(BODY_GAP))
                    .child(
                        div().text_color(rgb(color::MUTED)).child(
                            "Connect to a computer that runs Herdr over SSH. This iPad checks the host key before it signs in.",
                        ),
                    )
                    .child(
                        step()
                            .child(step_title(1, key_ready, !key_ready, "Allow this iPad on the host"))
                            .child(div().ml(px(STEP_INDENT)).child(key)),
                    )
                    .child(
                        step()
                            .child(step_title(2, host_done, !host_done, "Check the host key"))
                            .child(check),
                    )
                    .child(
                        step()
                            .when(!host_done, |step| step.opacity(UPCOMING_STEP_OPACITY))
                            .child(step_title(3, false, host_done, "Sign in and open Herdr"))
                            .child(sign_in),
                    ),
            )
            .into_any_element()
    }

    fn notice_block(&self, lines: Vec<String>, id: &'static str) -> AnyElement {
        div()
            .id(id)
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(rgb(0xbf616a))
            .children(lines.into_iter().map(|line| div().child(line)))
            .into_any_element()
    }
}

#[cfg(all(test, not(target_os = "ios")))]
#[path = "open_key_tests.rs"]
mod open_key_tests;
