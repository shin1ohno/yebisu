use crate::{app::Smoke, core::Command};
use gpui::{AnyElement, Context, div, prelude::*, rgb};

impl Smoke {
    pub(crate) fn connect_core(&mut self, cx: &mut Context<Self>) {
        if self.setup.busy || self.terminal.reconnect.suspended {
            return;
        }
        let host = if self.terminal.open {
            self.terminal.reconnect.target.clone()
        } else {
            self.setup_input.text.trim().to_owned()
        };
        self.select_host(&host);
        self.terminal.core.take();
        let generation = self.terminal.link.reset();
        self.terminal.core_mode = true;
        self.terminal.reconnect.begin(host.clone());
        self.terminal.input.detach();
        self.terminal.live = crate::live::Live::default();
        self.terminal.size = None;
        self.terminal.open = true;
        self.terminal.needs_focus = true;
        self.terminal.reconnect_pending = false;
        let inbox = self.terminal.link.inbox.clone();
        #[cfg(target_os = "ios")]
        let root = sheprd_client::local_path::state_dir().map(|p| p.join("herdr2"));
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let root = if std::env::var_os("YEBISU_CORE_QA").is_some() {
            // A mock daemon lives for this process only. Keep its saved workspace
            // across reconnects, separate from real hosts and previous QA runs.
            root.map(|p| p.join(format!("qa-{}", std::process::id())))
        } else {
            root
        };
        #[cfg(not(target_os = "ios"))]
        let root = None;
        let target = host.clone();
        let result = crate::core::Session::start(
            host,
            generation,
            inbox,
            move || {
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                if std::env::var_os("YEBISU_CORE_QA").is_some() {
                    return crate::core_mock::connect();
                }
                #[cfg(target_os = "ios")]
                {
                    crate::core::connect(&target)
                }
                #[cfg(not(target_os = "ios"))]
                {
                    Err(crate::Error::Ssh(format!(
                        "SSH unavailable in host test: {target}"
                    )))
                }
            },
            root,
        );
        match result {
            Ok(session) => self.terminal.core = Some(session),
            Err(e) => self.terminal.live.status = e.to_string(),
        }
        cx.notify();
    }
    pub(crate) fn paste_core(
        &mut self,
        destination: crate::terminal_input::Destination,
        image_only: bool,
        cx: &mut Context<Self>,
    ) {
        if image_only {
            self.terminal.live.status =
                "Herdr 2 accepts text paste; image paste requires Herdr 1".into();
            cx.notify();
            return;
        }
        #[cfg(test)]
        {
            self.handoff.clipboard_reads += 1;
        }
        match crate::handoff_clipboard::read(false, cx) {
            Ok(crate::handoff::Payload::Text(text)) if text.len() <= crate::handoff::TEXT_LIMIT => {
                // Acquisition is bounded on the UI thread; mode-aware encoding
                // and wire publication run in the same FIFO worker as keys.
                self.send_terminal(crate::terminal_input::GeneratedInput {
                    destination,
                    event: sheprd_backend::ClientPaneInputEvent::Paste(text),
                });
            }
            Ok(_) => {
                self.terminal.live.status =
                    "Herdr 2 paste needs text within the clipboard limit".into()
            }
            Err(error) => self.terminal.live.status = error.to_string(),
        }
        cx.notify();
    }
    fn core_command(&mut self, command: Command) {
        self.terminal.input.detach();
        self.handoff.cancel();
        if let Some(core) = &self.terminal.core
            && let Err(e) = core.send(command)
        {
            self.terminal.live.status = e.to_string();
        }
    }
    pub(crate) fn render_core_controls(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .gap_4()
            .px_4()
            .py_2()
            .bg(rgb(0x3b4252))
            .child(
                div()
                    .id("core-split")
                    .child("Split (2 panes)")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Split);
                        cx.notify();
                    })),
            )
            .child(div().id("core-first").child("Pane 1").on_click(cx.listener(
                |this, _, _, cx| {
                    this.core_command(Command::Focus(0));
                    cx.notify();
                },
            )))
            .child(
                div()
                    .id("core-second")
                    .child("Pane 2")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Focus(1));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("core-orientation")
                    .child("Toggle split direction")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Orientation);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("core-detach")
                    .child("Detach")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.terminal.core.take();
                        this.terminal.link.reset();
                        this.terminal.input.detach();
                        this.terminal.live = crate::live::Live::default();
                        this.terminal.live.status = "Detached · tap Reconnect to reattach".into();
                        window.blur(cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
