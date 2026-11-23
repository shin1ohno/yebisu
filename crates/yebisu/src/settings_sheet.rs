mod view;
pub(crate) use view::button;
// Modal adapter shared by production sheet and nested confirmation.
use crate::app::Smoke;
use gpui::{Context, Window};

impl Smoke {
    pub(crate) fn install_settings_shortcut(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.settings_shortcut.is_some() {
            return;
        }
        // The terminal may have no focus while disconnected or covered by an overlay.
        let owner = window.window_handle();
        let view = cx.entity().downgrade();
        self.settings_shortcut = Some(cx.intercept_keystrokes(move |event, window, app| {
            if window.window_handle() != owner
                || !crate::terminal_controls::is_settings_shortcut(&event.keystroke)
            {
                return;
            }
            let _ = view.update(app, |this, cx| {
                // If the terminal owns the key path, its handler opens Settings once.
                let terminal_receives_key = this.terminal.open
                    && this.shell.fit.too_small.is_none()
                    && this.terminal_focus.is_focused(window);
                if this.actions.herdr_prefix && !terminal_receives_key {
                    if !this.settings_modal {
                        this.open_settings_from_shortcut(window, cx);
                    }
                    cx.stop_propagation();
                    window.prevent_default();
                }
            });
        }));
    }

    pub(crate) fn begin_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_modal {
            return;
        }
        self.settings_modal = true;
        self.actions.host_keys.clear_press();
        if let Some(core) = &self.terminal.core {
            core.invalidate_input();
        }
        self.settings_focus.focus(window, cx);
        if let Some(host) = &mut self.settings {
            host.keys.view_open();
        }
        self.key_ticket = None;
        self.read_key_on_page_open(cx);
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
                    this.complete_settings_font_enumeration(fonts, cx);
                });
            })
            .detach();
        }
        self.actions.palette = false;
        self.actions.consumed.clear();
        self.terminal.input.detach();
        // The sheet takes text input away from the terminal; a late UIKit
        // commit after it closes stays fenced until a fresh key press
        // (settings_sheet/tests.rs, sh1_156_tap_tests.rs). A detach alone no
        // longer invents a discard (SH1-156 D1).
        self.terminal.input.fence();
        self.handoff.sync(None);
        self.handoff.cancel();
        self.handoff.select_mode = false;
        self.terminal.needs_focus = false;
        window.notify_text_input_changed(&self.terminal_focus);
        cx.notify();
    }

    /// The page with the iPad's SSH public key reads it when it opens, as
    /// Add host does (shell/hosts_sheet/actions.rs): on the device-key
    /// worker, never this thread, with the page's usual failure line. The
    /// page's Read button stays as the retry. A read still running for an
    /// earlier opening cannot show its key here (key_ops.rs drops it), so
    /// this opening reads as soon as that one ends.
    pub(crate) fn read_key_on_page_open(&mut self, cx: &mut Context<Self>) {
        if self.settings_page != crate::settings_pages::Page::DeviceKey {
            return;
        }
        let earlier = self.settings.as_ref().is_some_and(|host| host.keys.busy());
        if !earlier {
            self.read_device_key(cx);
        } else if self.key_provider_preflight("read")
            && let Some(host) = &mut self.settings
        {
            host.keys.read_after_earlier_view();
        }
    }

    pub(crate) fn complete_settings_font_enumeration(
        &mut self,
        fonts: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.settings_fonts = fonts;
        self.settings_fonts_loading = false;
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
            // GPUI focus alone did not give the native text responder back:
            // after Settings closed, keys typed without touching the terminal
            // were lost until a tap (owner device check of 153c583, R7/R8).
            // Ask for it as a tap does (handoff_view.rs).
            self.request_terminal_keyboard(window, "settings-close");
            // The sheet's fence waits for a fresh key press, which a software
            // keyboard never sends; native retirement releases it without a
            // tap (TerminalInput::fence, native_retirement.rs).
            self.request_text_retirement();
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

#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
mod observer_qa;
#[cfg(all(target_os = "ios", target_abi = "sim", not(feature = "test-support")))]
mod qa;

#[cfg(test)]
mod preflight_tests;
#[cfg(test)]
mod publication_tests;

#[cfg(test)]
mod publication_reproducers;

#[cfg(test)]
#[path = "settings_sheet/tests/wave_one.rs"]
mod wave_one_tests;

#[cfg(test)]
#[path = "settings_sheet/tests/key_on_open.rs"]
mod key_on_open_tests;

#[cfg(test)]
#[path = "settings_sheet/tests/window_controls.rs"]
mod window_controls_tests;
