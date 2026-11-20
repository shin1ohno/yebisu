use crate::{
    app::Smoke,
    core::{Command, SessionTarget},
    shell::HerdrKind,
};
use gpui::{AnyElement, Context, div, prelude::*, rgb};

impl Smoke {
    fn core_host(&self) -> String {
        if self.terminal.open {
            self.terminal.reconnect.target.clone()
        } else {
            self.setup_input.text.trim().to_owned()
        }
    }

    pub(crate) fn connect_core(&mut self, cx: &mut Context<Self>) {
        self.remember_connected_core();
        let host = self.core_host();
        let session = self.shell.herdr2.selected_session(&host).to_owned();
        let _ = self.connect_named_core(host, session, None, cx);
    }

    pub(crate) fn remember_connected_core(&mut self) {
        if self.terminal.live.connected
            && let Some(core) = &self.terminal.core
        {
            self.shell
                .herdr2
                .confirm_session(&self.terminal.reconnect.target, core.name());
        }
    }

    pub(crate) fn check_space_connection(&self) -> crate::Result<()> {
        let reason = if self.setup.busy {
            "Wait for host setup to finish before connecting"
        } else if self.terminal.reconnect.suspended {
            "Resume the app before connecting"
        } else {
            return Ok(());
        };
        Err(crate::Error::Ssh(reason.into()))
    }

    pub(crate) fn connect_named_core(
        &mut self,
        host: String,
        name: String,
        focus: Option<String>,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        self.connect_core_target(
            SessionTarget {
                host,
                name,
                focus,
                expected_core_id: None,
            },
            cx,
        )
    }

    pub(crate) fn connect_core_target(
        &mut self,
        target: SessionTarget,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let result = self.start_named_core(target, cx);
        if let Err(error) = &result {
            self.terminal.live.status = error.to_string();
            cx.notify();
        }
        result
    }

    fn start_named_core(
        &mut self,
        target: SessionTarget,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        self.check_space_connection()?;
        self.remember_connected_core();
        #[cfg(test)]
        if let Some(backend) = self.terminal.core_test_backend.take() {
            return self.input_transition(|this| {
                this.connect_core_using_inner(target, cx, move |_, _| Ok(backend))
            });
        }
        self.input_transition(|this| {
            this.connect_core_using_inner(target, cx, |host, session| {
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                if std::env::var_os("YEBISU_CORE_QA").is_some() {
                    return crate::core_mock::connect();
                }
                #[cfg(target_os = "ios")]
                {
                    crate::core::connect(host, session)
                }
                #[cfg(not(target_os = "ios"))]
                {
                    Err(crate::Error::Ssh(format!(
                        "SSH unavailable in host test: {host}/{session}"
                    )))
                }
            })
        })
    }

    #[cfg(test)]
    pub(crate) fn connect_core_using(
        &mut self,
        cx: &mut Context<Self>,
        connect: impl FnOnce(&str) -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
    ) {
        let target = SessionTarget {
            host: self.core_host(),
            name: "default".into(),
            focus: None,
            expected_core_id: None,
        };
        let result = self.check_space_connection().and_then(|()| {
            self.input_transition(|this| {
                this.connect_core_using_inner(target, cx, move |host, _| connect(host))
            })
        });
        if let Err(error) = result {
            self.terminal.live.status = error.to_string();
            cx.notify();
        }
    }

    fn connect_core_using_inner(
        &mut self,
        target: SessionTarget,
        cx: &mut Context<Self>,
        connect: impl FnOnce(&str, &str) -> crate::Result<sheprd_core_client::CoreBackend>
        + Send
        + 'static,
    ) -> crate::Result<()> {
        let host = target.host.clone();
        let name = target.name.clone();
        self.select_host_kind(&host, HerdrKind::Herdr2);
        self.terminal.core.take();
        self.terminal.navigating = None;
        self.actions.palette = false;
        let generation = self.terminal.link.reset_for_core();
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
        let root = crate::core::workspace_root();
        #[cfg(not(target_os = "ios"))]
        let root = None;
        let connect_host = host.clone();
        let connect_name = name.clone();
        let column = self.shell.single_pane.mailbox();
        let session = crate::core::Session::start_named(
            target,
            generation,
            inbox,
            move || connect(&connect_host, &connect_name),
            root,
            column,
        )?;
        self.terminal.core = Some(session);
        cx.notify();
        Ok(())
    }
    /// Shared by the Detach control and its real-adapter regression.
    pub(crate) fn detach_core_and_blur(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.detach_core();
        window.blur(cx);
        cx.notify();
    }
    pub(crate) fn detach_core(&mut self) {
        self.input_transition(|this| this.detach_core_inner())
    }
    fn detach_core_inner(&mut self) {
        self.remember_connected_core();
        self.handoff.cancel();
        self.terminal.core.take();
        self.terminal.link.reset();
        self.terminal.input.detach();
        self.terminal.live = crate::live::Live::default();
        self.terminal.live.status = "Detached · tap Reconnect to reattach".into();
        self.terminal.needs_focus = false;
        self.sync_handoff();
    }
    pub(crate) fn paste_core(
        &mut self,
        destination: crate::terminal_input::Destination,
        image_only: bool,
        cx: &mut Context<Self>,
    ) {
        if self.new_space_open() {
            return;
        }
        #[cfg(test)]
        {
            self.handoff.clipboard_reads += 1;
        }
        match crate::handoff_clipboard::read(image_only, cx) {
            Ok(crate::handoff::Payload::Text(text)) if text.len() <= crate::handoff::TEXT_LIMIT => {
                // Acquisition is bounded on the UI thread; mode-aware encoding
                // and wire publication run in the same FIFO worker as keys.
                self.send_terminal(crate::terminal_input::GeneratedInput {
                    destination,
                    event: sheprd_backend::ClientPaneInputEvent::Paste(text),
                });
                crate::qa_ax::core::count("pasted_texts");
            }
            Ok(payload @ crate::handoff::Payload::Image(_)) => self.start_core_image(payload, cx),
            Ok(_) => {
                self.terminal.live.status =
                    "Herdr 2 paste needs text within the clipboard limit".into()
            }
            Err(error) => self.terminal.live.status = crate::core_paste_view::paste_error(error),
        }
        cx.notify();
    }
    pub(crate) fn core_command(&mut self, command: Command) {
        if let Err(error) = self.try_core_command(command) {
            self.terminal.live.status = error.to_string();
        }
    }

    pub(crate) fn try_core_command(&mut self, command: Command) -> crate::Result<()> {
        self.input_transition(|this| {
            if this.input_modal() || this.new_space_open() || this.terminal.reconnect.suspended {
                return Err(crate::Error::Ssh("Herdr 2 command unavailable".into()));
            }
            let core = this
                .terminal
                .core
                .as_ref()
                .ok_or_else(|| crate::Error::Ssh("Herdr 2 is not connected".into()))?;
            core.invalidate_input();
            this.terminal.input.detach();
            this.handoff.cancel();
            this.handoff.selection = None;
            core.send(command)
        })
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
                    .child(crate::shell::herdr2_frame::panes::SPLIT)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Split);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("core-first")
                    .child(crate::shell::herdr2_frame::panes::PANE_1)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Focus(0));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("core-second")
                    .child(crate::shell::herdr2_frame::panes::PANE_2)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Focus(1));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("core-orientation")
                    .child(crate::shell::herdr2_frame::panes::ORIENTATION)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.core_command(Command::Orientation);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("core-detach")
                    .child(crate::shell::herdr2_frame::panes::DETACH)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.detach_core_and_blur(window, cx);
                    })),
            )
            .into_any_element()
    }
}
