//! Modal adapter, independent of the not-yet-accepted responsive sheet geometry.
use crate::app::Smoke;
use gpui::{Context, Window};

impl Smoke {
    #[allow(
        dead_code,
        reason = "production route waits for accepted responsive contract"
    )]
    pub(crate) fn begin_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_modal {
            return;
        }
        self.settings_modal = true;
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

    #[allow(
        dead_code,
        reason = "production route waits for accepted responsive contract"
    )]
    pub(crate) fn end_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_modal {
            return;
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
