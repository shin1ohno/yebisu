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
        if matches!(&job, Job::Key) {
            self.setup.key_line = None;
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
                if let Ok(Reply::Loaded(target, _)) = &result {
                    this.input.set_text(target.clone());
                }
                this.setup.apply(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn render_setup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.observation.record(serde_json::json!({
            "kind": "setup", "busy": self.setup.busy, "status": self.setup.status,
            "target": self.input.text, "fingerprints": self.setup.fingerprints,
            "key_line": self.setup.key_line,
        }));
        let input_entity = cx.entity();
        let busy = self.setup.busy;
        let key_ready = self.setup.key_line.is_some();
        let pending = self.setup.pending.is_some();
        div()
            .id("keys-hosts")
            .size_full()
            .bg(rgb(0x2e3440))
            .text_color(rgb(0xeceff4))
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_4()
            .p_8()
            .track_focus(&self.focus)
            .child(div().text_3xl().child("yebisu · Keys and hosts · P2"))
            .child(div().child("1. Create this installation's SSH key"))
            .child(
                button("key-create", "Create / show public key", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.start_job(Job::Key, cx);
                    },
                )),
            )
            .child(
                div().text_sm().w_full().overflow_hidden().child(
                    self.setup
                        .key_line
                        .clone()
                        .unwrap_or_else(|| "No public key loaded.".into()),
                ),
            )
            .child(
                button("key-copy", "Copy authorized_keys line", busy || !key_ready).on_click(
                    cx.listener(|this, _, _, cx| {
                        if !this.setup.busy
                            && let Some(line) = &this.setup.key_line
                        {
                            cx.write_to_clipboard(ClipboardItem::new_string(line.clone()));
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
                    .child(if self.input.text.is_empty() {
                        "user@host[:port]".to_owned()
                    } else {
                        self.input.text.clone()
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
                        this.start_job(Job::Scan(this.input.text.trim().to_owned()), cx);
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
                        && target == this.input.text.trim()
                    {
                        this.start_job(Job::Trust(target, key), cx);
                    }
                })),
            )
            .child(
                button("host-auth", "Check SSH authentication", busy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.start_job(Job::Authenticate(this.input.text.trim().to_owned()), cx);
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
