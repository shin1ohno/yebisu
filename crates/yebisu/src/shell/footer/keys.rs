//! L2b: hardware keys while a sheet the footer opened is on screen.
//!
//! Review cf8bbcccc2164abe F1 and amendment A6: the Hosts sheet opens over a
//! connected terminal, and the terminal's key handler and text input follow
//! `terminal_focus`. While the sheet is open the keys belong to the sheet,
//! so footer Hosts moves the window focus to the sheet's own focus handle
//! (`Smoke::focus`, the handle the sheet tracks and its host field reads)
//! and does the clean-up `begin_settings_modal` does. The terminal's
//! `on_key_down` sits on the `terminal_focus` path, and the terminal
//! registers its text input only while `terminal_focus` is focused, so
//! neither KeyDown nor typed text reaches the remote shell. Closing the
//! sheet gives the focus and the keyboard back as `end_settings_modal` does.
//! `input_modal` is the guard the integrator patch puts where input code
//! checked only `settings_modal` (A6 item 1).
use crate::app::Smoke;
use gpui::{Context, Window};

/// Who receives a hardware key now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyOwner {
    Terminal,
    HostsSheet,
    NewSpaceSheet,
    SettingsSheet,
}

impl Smoke {
    pub(crate) fn key_owner(&self) -> KeyOwner {
        if self.settings_modal {
            KeyOwner::SettingsSheet
        } else if self.hosts_sheet_modal() {
            KeyOwner::HostsSheet
        } else if self.new_space_open() {
            KeyOwner::NewSpaceSheet
        } else {
            KeyOwner::Terminal
        }
    }

    /// Whether a sheet owns input over the terminal.
    pub(crate) fn input_modal(&self) -> bool {
        self.key_owner() != KeyOwner::Terminal
    }

    /// Whether typed text (IME, `insertText:`) belongs to the terminal: a
    /// terminal is open and neither Hosts nor New space covers it.
    pub(crate) fn terminal_owns_text_input(&self) -> bool {
        self.terminal.open && !self.hosts_sheet_modal() && !self.new_space_open()
    }

    /// Footer Hosts opened the sheet: give it the window focus and take the
    /// terminal's input away, as `begin_settings_modal` does (A6 item 2).
    pub(crate) fn hand_keys_to_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.key_owner() != KeyOwner::HostsSheet || self.terminal_owns_text_input() {
            return;
        }
        if let Some(core) = &self.terminal.core {
            core.invalidate_input();
        }
        self.actions.palette = false;
        self.actions.consumed.clear();
        self.terminal.input.detach();
        self.terminal.input.fence();
        self.handoff.sync(None);
        self.handoff.cancel();
        self.handoff.select_mode = false;
        self.terminal.needs_focus = false;
        self.focus.focus(window, cx);
        window.notify_text_input_changed(&self.terminal_focus);
    }

    /// The Hosts sheet over a terminal closed: the terminal gets the focus
    /// and the keyboard back, as after `end_settings_modal` (A6 item 3).
    pub(crate) fn return_keys_to_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.key_owner() != KeyOwner::Terminal {
            return;
        }
        self.refresh_terminal_input();
        if self.terminal.open && self.terminal.input.ready {
            self.terminal_focus.focus(window, cx);
            self.request_terminal_keyboard(window, "hosts-close");
            self.request_text_retirement();
        } else if let Some(bar) = &self.find {
            // The find bar was open under the sheet: its field takes the keys
            // again, as when the bar opened (find_view.rs; review 87cd S1).
            bar.focus_handle().focus(window, cx);
            window.request_virtual_keyboard();
        } else {
            self.focus.focus(window, cx);
        }
        window.notify_text_input_changed(self.active_focus());
        cx.notify();
    }
}
