//! Drawing for the first-run frame, the Hosts sheet and the Add host sheet.
//! Colours and sizes follow canvas AddHost.dc.html; no rule lives here.
use super::{DEVELOPER_BUTTONS, developer_items};
use crate::{
    app::Smoke,
    layout::SIDEBAR_WIDTH,
    setup::Job,
    shell::add_host::{AddHostStep, SheetMode, sheet_height, sheet_width},
};
use gpui::{
    AnyElement, Context, ElementId, MouseButton, Role, SharedString, Stateful, Window, canvas, div,
    prelude::*, px, rgb, rgba,
};

const PANEL: u32 = 0x1c1c22;
const BORDER: u32 = 0x2b2933;
const INK: u32 = 0xd8dee9;
const MUTED: u32 = 0xacb2bb;
const ACCENT: u32 = 0x88c0d0;
const GREEN: u32 = 0xa3be8c;
const FIELD: u32 = 0x101419;

fn button(
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
        .min_h(px(44.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .rounded(px(8.))
        .bg(rgb(if primary && enabled { ACCENT } else { BORDER }))
        .text_color(rgb(if primary && enabled {
            FIELD
        } else if enabled {
            INK
        } else {
            MUTED
        }))
        .when(!enabled, |button| button.opacity(0.45))
        .child(label)
}

fn badge(number: u8, done: bool, active: bool) -> impl IntoElement {
    div()
        .size(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .font_weight(gpui::FontWeight::BOLD)
        .when(done, |d| d.bg(rgb(GREEN)).text_color(rgb(FIELD)))
        .when(!done && active, |d| {
            d.bg(rgb(ACCENT)).text_color(rgb(FIELD))
        })
        .when(!done && !active, |d| {
            d.border_2().border_color(rgb(MUTED)).text_color(rgb(MUTED))
        })
        .child(number.to_string())
}

fn step_title(number: u8, done: bool, active: bool, title: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(badge(number, done, active))
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
}

impl Smoke {
    fn record_setup_observation(&self) {
        self.observation.record(serde_json::json!({
            "kind": "setup", "busy": self.setup.busy, "status": self.setup.status,
            "target": self.setup_input.text, "fingerprints": self.setup.fingerprints,
            "key_ready": self.settings.as_ref().is_some_and(|h|h.keys.line().is_some()), "hosts": self.setup.hosts,
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
        self.shell
            .hosts_sheet
            .auto_open(self.setup.loaded, self.setup.hosts.len());
        self.record_setup_observation();
        let sidebar_width = SIDEBAR_WIDTH.min(f32::from(window.viewport_size().width));
        let hosts = self.setup.hosts.clone();
        let selected = self.setup.target.clone();
        let mut list = div().id("first-run-hosts").flex().flex_col().gap_1().p_2();
        for (index, host) in hosts.into_iter().enumerate() {
            let target = host.clone();
            list = list.child(
                button(
                    format!("first-run-host-{index}"),
                    host.clone(),
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
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER))
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
                            .text_color(rgb(MUTED))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
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
                .children(self.key_qa_label())
                .size_full()
                .flex()
                .bg(rgb(FIELD))
                .text_color(rgb(INK))
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
        if !self.shell.hosts_sheet.visible() {
            return None;
        }
        if self.terminal.open {
            self.record_setup_observation();
        }
        let available = window.fully_visible_bounds();
        let width = sheet_width(f32::from(available.size.width));
        let keyboard = f32::from(window.insets().ime.bottom);
        let height = sheet_height(f32::from(available.size.height), keyboard);
        let body = if self.shell.hosts_sheet.add.is_some() {
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
            .bg(rgba(0x101419a8))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("hosts-panel")
                    .debug_selector(|| "hosts-panel".into())
                    .absolute()
                    .left(available.origin.x + px((f32::from(available.size.width) - width) / 2.))
                    .top(available.origin.y + px(16.))
                    .w(px(width))
                    .max_h(px(height))
                    .flex()
                    .flex_col()
                    .bg(rgb(PANEL))
                    .text_color(rgb(INK))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .rounded(px(18.))
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
            .border_color(rgb(BORDER))
            .child(div().w(px(104.)).flex().justify_start().child(left))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(div().w(px(104.)).flex().justify_end().child(right))
            .into_any_element()
    }

    /// The iPad's public key and its copy button (step 1 and the list sheet).
    fn key_block(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.setup.busy;
        let line = self
            .settings
            .as_ref()
            .and_then(|h| h.keys.line())
            .map(str::to_owned);
        let key_ready = line.is_some();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("device-key-line")
                    .w_full()
                    .overflow_hidden()
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(FIELD))
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .font_family("SauceCodePro Nerd Font Mono")
                    .child(line.unwrap_or_else(|| "No public key loaded.".into())),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        button("key-create", "Read or initialize device key", !busy, false)
                            .on_click(cx.listener(|this, _, _, cx| this.read_device_key(cx))),
                    )
                    .child(
                        button(
                            "key-copy",
                            "Copy authorized_keys line",
                            !busy && key_ready,
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.setup.busy && this.copy_device_key(cx) {
                                this.setup.status = "Copied restricted public key.".into();
                                cx.notify();
                            }
                        })),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("Add it to ~/.ssh/authorized_keys on the host."),
            )
            .into_any_element()
    }

    fn hosts_list_sheet(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.setup.busy;
        let mut rows = div().id("hosts-rows").flex().flex_col().gap_2();
        let hosts = self.setup.hosts.clone();
        if hosts.is_empty() {
            rows = rows.child(div().text_color(rgb(MUTED)).child("No hosts yet."));
        }
        for (index, host) in hosts.into_iter().enumerate() {
            let open = host.clone();
            let edit = host.clone();
            rows = rows.child(
                div()
                    .id(("hosts-row", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(host.clone()),
                    )
                    .child(
                        button(format!("host-open-{index}"), "Open", !busy, true).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.hosts_open_add(SheetMode::Open, &open, cx);
                            }),
                        ),
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
            .on_click(cx.listener(|this, _, _, cx| this.hosts_close(cx)))
            .into_any_element();
        let add = button("hosts-add", "Add host", !busy, true)
            .on_click(cx.listener(|this, _, _, cx| this.hosts_open_add(SheetMode::Add, "", cx)))
            .into_any_element();
        let header = self.sheet_header("Hosts", close, add);
        let key = self.key_block(cx);
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
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("This iPad"),
                    )
                    .child(key)
                    .child(
                        div()
                            .id("setup-status")
                            .text_sm()
                            .text_color(rgb(MUTED))
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
        let key_ready = self
            .settings
            .as_ref()
            .is_some_and(|h| h.keys.line().is_some());
        let scanning = flow.scanning && flow.step == AddHostStep::Address;
        let trusting = flow.scanning && flow.step == AddHostStep::Trust;
        let host_done = flow.step == AddHostStep::SignIn;
        let input_entity = cx.entity();
        let field = div()
            .id("target-field")
            .flex_1()
            .min_w_0()
            .h(px(48.))
            .px_3()
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(ACCENT))
            .bg(rgb(FIELD))
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
                .on_click(cx.listener(|this, _, _, cx| this.hosts_cancel_scan(cx)))
        } else {
            button("host-scan", "Scan host key", flow.can_scan(), false)
                .on_click(cx.listener(|this, _, _, cx| this.hosts_scan(cx)))
        };
        let mut check = div()
            .ml(px(34.))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(MUTED)).child("Host"))
            .child(div().flex().gap_2().child(field).child(scan_control));
        if scanning {
            check = check.child(div().id("scan-progress").child("Scanning…"));
        }
        if let (AddHostStep::Trust, Some(fingerprint)) = (flow.step, &flow.scanned_fingerprint) {
            check = check.child(
                div()
                    .id("fingerprint-box")
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
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
                                button("host-reject", "Not this host", !trusting, false).on_click(
                                    cx.listener(|this, _, _, cx| this.hosts_reject_key(cx)),
                                ),
                            )
                            .child(
                                button(
                                    "host-trust",
                                    "Trust the compared fingerprint and save",
                                    flow.can_trust(),
                                    true,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.hosts_trust(cx))),
                            ),
                    ),
            );
        }
        if let Some(notice) = session.notice.as_ref().filter(|_| !host_done) {
            check = check.child(self.notice_block(notice.lines(), "notice-host-key"));
        }
        let mut sign_in = div()
            .ml(px(34.))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(MUTED)).child(
            "Checks SSH sign-in, then opens Herdr 1. If only Herdr 2 runs there, it opens Herdr 2.",
        ));
        if session.signing_in {
            sign_in = sign_in.child(div().id("sign-in-progress").child("Signing in…"));
        }
        if host_done {
            let retry = session.notice.as_ref().is_some_and(|n| n.offers_retry());
            sign_in = sign_in.child(
                button(
                    "host-auth",
                    if retry {
                        "Retry"
                    } else {
                        "Sign in and open Herdr"
                    },
                    session.can_sign_in(),
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| this.hosts_sign_in(cx))),
            );
            if let Some(notice) = &session.notice {
                sign_in = sign_in.child(self.notice_block(notice.lines(), "notice-sign-in"));
            }
        }
        let cancel = button("hosts-add-cancel", "Cancel", true, false)
            .on_click(cx.listener(|this, _, _, cx| this.hosts_close(cx)))
            .into_any_element();
        let connect = button("hosts-add-connect", "Connect", session.can_sign_in(), false)
            .on_click(cx.listener(|this, _, _, cx| this.hosts_sign_in(cx)))
            .into_any_element();
        let header = self.sheet_header(session.mode.title(), cancel, connect);
        let key = self.key_block(cx);
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
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().text_color(rgb(MUTED)).child(
                            "Connect to a computer that runs Herdr over SSH. This iPad checks the host key before it signs in.",
                        ),
                    )
                    .child(step_title(1, key_ready, !key_ready, "Allow this iPad on the host"))
                    .child(div().ml(px(34.)).child(key))
                    .child(step_title(2, host_done, !host_done, "Check the host key"))
                    .child(check)
                    .child(step_title(3, false, host_done, "Sign in and open Herdr"))
                    .child(sign_in)
                    .child(
                        div()
                            .id("setup-status")
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(self.setup.status.clone()),
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
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(0xbf616a))
            .children(lines.into_iter().map(|line| div().child(line)))
            .into_any_element()
    }
}
