// Keep trust confirmation bound to the scanned target.
use crate::{
    app::Smoke,
    setup::{Job, Reply},
};
use gpui::{AnyElement, ClipboardItem, Context, canvas, div, prelude::*, rgb};

impl Smoke {
    pub(crate) fn start_job(&mut self, job: Job, cx: &mut Context<Self>) {
        if self.setup.busy {
            return;
        }
        self.setup.busy = true;
        self.setup.pending = None;
        self.setup.status = "Working…".into();
        let work = cx
            .background_executor()
            .spawn(async move { crate::setup::execute(job) });
        cx.spawn(async move |weak, cx| {
            let result = work.await;
            let _ = weak.update(cx, |this, cx| {
                this.apply_setup_result(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn apply_setup_result(&mut self, result: crate::Result<Reply>) {
        if let Ok(Reply::Loaded(target, _)) = &result {
            self.setup_input.set_text(target.clone());
        }
        if let Ok(Reply::Hosts(hosts, _)) = &result {
            self.setup_input.set_text(hosts.selected.clone());
        }
        self.setup.apply(result);
    }

    pub(crate) fn render_setup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.observation.record(serde_json::json!({
            "kind": "setup", "busy": self.setup.busy, "status": self.setup.status,
            "target": self.setup_input.text, "fingerprints": self.setup.fingerprints,
            "key_ready": self.settings.as_ref().is_some_and(|h|h.keys.line().is_some()), "hosts": self.setup.hosts,
        }));
        let input_entity = cx.entity();
        let busy = self.setup.busy;
        let key_ready = self
            .settings
            .as_ref()
            .is_some_and(|h| h.keys.line().is_some());
        let pending = self.setup.pending.is_some();
        div()
            .id("keys-hosts")
            .size_full()
            .bg(rgb(0x2e3440))
            .text_color(rgb(0xeceff4))
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .items_center()
            .gap_4()
            .p_8()
            .track_focus(&self.focus)
            .child(div().text_3xl().child("yebisu · Saved hosts · P9"))
            .child(self.settings_button(cx))
            .child(
                button("core-connect", "Open Herdr 2", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.connect_core(cx);
                    },
                )),
            )
            .children(
                self.setup
                    .hosts
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(i, target)| {
                        div()
                            .id(("saved-host", i))
                            .child(target.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.setup.busy {
                                    this.setup_input.set_text(target.clone());
                                    this.setup.pending = None;
                                    this.setup.fingerprints.clear();
                                    cx.notify();
                                }
                            }))
                    }),
            )
            .child(
                button("host-save", "Save connection", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.start_job(Job::Save(this.setup_input.text.trim().to_owned()), cx);
                    },
                )),
            )
            .child(div().child("1. Read or initialize this installation's SSH key"))
            .child(
                button("key-create", "Read or initialize device key", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.read_device_key(cx);
                    },
                )),
            )
            .child(
                div().text_sm().w_full().overflow_hidden().child(
                    self.settings
                        .as_ref()
                        .and_then(|h| h.keys.line())
                        .map(str::to_owned)
                        .unwrap_or_else(|| "No public key loaded.".into()),
                ),
            )
            .child(
                button("key-copy", "Copy authorized_keys line", busy || !key_ready).on_click(
                    cx.listener(|this, _, _, cx| {
                        if !this.setup.busy
                            && let Some(line) = this.settings.as_ref().and_then(|h| h.keys.line())
                        {
                            cx.write_to_clipboard(ClipboardItem::new_string(line.to_owned()));
                            this.setup.status = "Copied restricted public key.".into();
                            cx.notify();
                        }
                    }),
                ),
            )
            .child(div().child("2. Enter user@host[:port] and compare the host fingerprint"))
            .child(
                div()
                    .id("target-field")
                    .w_full()
                    .h_12()
                    .p_2()
                    .bg(rgb(0x3b4252))
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
                    ),
            )
            .child(
                button("host-scan", "Scan host key", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.setup.fingerprints.clear();
                        this.start_job(Job::Scan(this.setup_input.text.trim().to_owned()), cx);
                    },
                )),
            )
            .child(div().text_sm().child(self.setup.fingerprints.clone()))
            .child(
                button(
                    "host-trust",
                    "Trust the compared fingerprint and save",
                    busy || !pending,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if !this.setup.busy
                        && let Some((target, key)) = this.setup.pending.take()
                        && target == this.setup_input.text.trim()
                    {
                        this.start_job(Job::Trust(target, key), cx);
                    }
                })),
            )
            .child(
                button("host-auth", "Check SSH authentication", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.start_job(
                            Job::Authenticate(this.setup_input.text.trim().to_owned()),
                            cx,
                        );
                    },
                )),
            )
            .child(
                button("terminal-connect", "Open terminal", busy).on_click(cx.listener(
                    |this, _, window, cx| {
                        window.dismiss_virtual_keyboard();
                        this.connect_terminal(cx);
                    },
                )),
            )
            .child(
                div()
                    .id("setup-status")
                    .text_sm()
                    .child(self.setup.status.clone()),
            )
            .child(
                button("save-failure", "Developer: simulate save failure", busy).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.start_job(Job::SaveFailure, cx);
                    }),
                ),
            )
            .child(
                div()
                    .id("smoke-open")
                    .text_sm()
                    .child("Developer: P0 rendering check")
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.setup.busy {
                            this.smoke_mode = true;
                            window.dismiss_virtual_keyboard();
                            cx.notify();
                        }
                    })),
            )
            .into_any_element()
    }
}

fn button(id: &'static str, label: &'static str, disabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_lg()
        .bg(rgb(if disabled { 0x434c5e } else { 0x5e81ac }))
        .child(label)
        .when(disabled, |element| element.opacity(0.45))
}
