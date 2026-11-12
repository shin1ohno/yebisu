mod view;
// Modal adapter shared by production sheet and nested confirmation.
use crate::app::Smoke;
use gpui::{Context, Window};

impl Smoke {
    pub(crate) fn begin_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_modal {
            return;
        }
        self.settings_modal = true;
        self.settings_focus.focus(window, cx);
        if let Some(host) = &mut self.settings {
            host.keys.view_open();
        }
        self.key_ticket = None;
        if self.settings_fonts.is_empty() && !self.settings_fonts_loading {
            self.settings_fonts_loading = true;
            let text_system = cx.text_system().clone();
            let work = cx.background_executor().spawn(async move {
                let mut fonts = text_system.all_font_names();
                fonts.retain(|family| {
                    crate::settings::font::TerminalFont {
                        size: None,
                        family: family.clone(),
                    }
                    .validate()
                    .is_ok()
                });
                fonts.sort();
                fonts.dedup();
                fonts
            });
            cx.spawn(async move |weak, cx| {
                let fonts = work.await;
                let _ = weak.update(cx, |this, cx| {
                    this.settings_fonts = fonts;
                    this.settings_fonts_loading = false;
                    cx.notify();
                });
            })
            .detach();
        }
        self.actions.palette = false;
        self.actions.consumed.clear();
        self.terminal.input.detach();
        self.handoff.sync(None);
        self.handoff.cancel();
        self.handoff.select_mode = false;
        self.terminal.needs_focus = false;
        window.notify_text_input_changed(&self.terminal_focus);
        cx.notify();
    }

    pub(crate) fn end_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_modal {
            return;
        }
        self.key_ticket = None;
        if let Some(host) = &mut self.settings {
            host.keys.view_open();
        }
        self.poll_settings(cx);
        self.settings_modal = false;
        // Recompute from current live state. No opening-time destination is retained.
        self.refresh_terminal_input();
        if self.terminal.open && self.terminal.input.ready {
            self.terminal_focus.focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        window.notify_text_input_changed(self.active_focus());
        cx.notify();
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod production_tests;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod qa;
