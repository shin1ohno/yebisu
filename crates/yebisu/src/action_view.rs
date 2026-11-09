use crate::{actions::catalog, app::Smoke, board::snapshot};
use gpui::{AnyElement, Context, div, prelude::*, rgb};
use sheprd_commands::Command;

impl Smoke {
    fn action_button(&self, command: Command, cx: &mut Context<Self>) -> AnyElement {
        let entry = catalog(&self.terminal)
            .into_iter()
            .find(|a| a.info.command == command);
        let Some(entry) = entry else {
            return div().into_any_element();
        };
        let enabled = entry.available;
        div()
            .id(entry.info.name)
            .px_3()
            .py_2()
            .bg(rgb(if enabled { 0x434c5e } else { 0x242933 }))
            .text_color(rgb(if enabled { 0xd8dee9 } else { 0x777777 }))
            .child(entry.label)
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    if let Err(e) = this.run_action(command) {
                        this.terminal.live.status = e.to_string();
                    }
                    window.notify_text_input_changed(&this.terminal_focus);
                    window.dismiss_virtual_keyboard();
                    cx.notify();
                }
            }))
            .into_any_element()
    }
    pub(crate) fn render_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let tabs = snapshot(&self.terminal.live)
            .map(|s| {
                s.tabs
                    .iter()
                    .filter(|t| Some(&t.workspace_id) == s.focused_workspace_id.as_ref())
                    .enumerate()
                    .map(|(index, t)| (index, t.tab_id.clone(), t.label.clone(), t.focused))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                div()
                    .id("tab-strip")
                    .flex()
                    .overflow_x_scroll()
                    .gap_2()
                    .px_4()
                    .children(tabs.into_iter().map(|(index, id, label, focused)| {
                        let boot = snapshot(&self.terminal.live)
                            .map(|s| s.boot_id.clone())
                            .unwrap_or_default();
                        let generation = self.terminal.link.generation;
                        let host = self.terminal.reconnect.target.clone();
                        div()
                            .id(("tab", index))
                            .px_3()
                            .py_2()
                            .bg(rgb(if focused { 0x5e81ac } else { 0x3b4252 }))
                            .child(format!("{} · {}", index + 1, label))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Err(e) = this.focus_tab(&host, &id, &boot, generation) {
                                    this.terminal.live.status = e.to_string();
                                }
                                window.dismiss_virtual_keyboard();
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div().flex().flex_wrap().gap_2().px_4().children(
                    [
                        Command::Tab,
                        Command::SplitRight,
                        Command::SplitDown,
                        Command::ToggleSidebar,
                        Command::Palette,
                    ]
                    .into_iter()
                    .map(|c| self.action_button(c, cx)),
                ),
            )
            .when(self.actions.palette, |view| {
                view.child(
                    div()
                        .id("action-palette")
                        .h(gpui::px(200.))
                        .overflow_y_scroll()
                        .px_4()
                        .children(
                            catalog(&self.terminal)
                                .into_iter()
                                .filter(|a| a.info.command != Command::Palette)
                                .map(|a| self.action_button(a.info.command, cx)),
                        ),
                )
            })
            .into_any_element()
    }
    pub(crate) fn focus_tab(
        &mut self,
        host: &str,
        id: &str,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        if self.terminal.reconnect.target != host || self.terminal.link.generation != generation {
            return Err(crate::Error::Ssh("Stale tab; tap again".into()));
        }
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host busy".into()))?;
        let s = snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        if !live.connected
            || live.surface.as_ref().is_some_and(|s| s.popup.is_some())
            || terminal.navigating.is_some()
            || s.boot_id != boot
            || !s
                .tabs
                .iter()
                .any(|t| t.tab_id == id && Some(&t.workspace_id) == s.focused_workspace_id.as_ref())
        {
            return Err(crate::Error::Ssh("Tab unavailable".into()));
        }
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = handle
            .focus_tab(boot, id)
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        terminal.navigating = Some(crate::board::Navigation {
            boot: boot.into(),
            revision: s.revision,
            target: crate::board::Target::Command,
            request: request.clone(),
        });
        terminal.input.detach();
        live.pending_request = Some(request);
        live.completed_request = None;
        live.failed_request = None;
        self.actions.palette = false;
        Ok(())
    }
}
