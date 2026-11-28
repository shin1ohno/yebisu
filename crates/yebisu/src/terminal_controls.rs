use crate::{
    actions::herdr_keys::Route, app::Smoke, settings_pages::toolbar_sync::ToolbarSetting,
    terminal_input::GeneratedInput,
};
use gpui::{
    AnyElement, Context, KeyDownEvent, KeyUpEvent, Keystroke, Window, div, prelude::*, rgb,
};

pub(crate) fn is_settings_shortcut(key: &Keystroke) -> bool {
    key.key == "," && key.modifiers.platform && key.modifiers.number_of_modifiers() == 1
}

impl Smoke {
    pub(crate) fn publish_native_edits(&mut self, window: &Window) {
        let availability = self.native_edit_availability(window);
        self.edit_menu_check(window);
        self.edit_menu
            .presenter
            .publish(window.window_handle(), availability);
    }

    pub(crate) fn native_edit_availability(
        &mut self,
        window: &Window,
    ) -> Option<crate::edit_menu::Availability> {
        self.refresh_terminal_input();
        self.sync_handoff();
        // While the search field holds focus, publish no window-wide policy:
        // the terminal's explicit Edit policy (Cut only with a terminal
        // selection) would otherwise refuse the field's own Cut, Copy and
        // Paste, and the SDK's default text-input rules apply instead.
        if !self.terminal.open
            || self
                .find
                .as_ref()
                .is_some_and(|bar| bar.owns_text_editing(window))
        {
            return None;
        }
        Some({
            let focused = !self.input_modal()
                && self.terminal_focus.is_focused(window)
                && self.terminal.navigating.is_none()
                && !self.actions.palette
                && !self.shell.shortcuts.open
                && !self.terminal.reconnect.suspended
                && self.handoff.stamp.is_some();
            let copy = focused
                && self
                    .handoff
                    .selection
                    .as_ref()
                    .and_then(crate::handoff::Selection::text)
                    .is_some();
            crate::edit_menu::Availability {
                copy,
                // Preserve D3: Cut copies terminal cells without deleting them.
                cut: copy,
                paste: focused
                    && self.terminal.input.ready
                    && self.handoff.workers == 0
                    && self
                        .handoff
                        .core_ticket
                        .as_ref()
                        .is_none_or(|ticket| ticket.is_finished()),
                // D3 has no terminal Select All implementation.
                select_all: false,
            }
        })
    }

    /// Ask UIKit for the terminal's text responder, as a tap on the terminal
    /// does; with a hardware keyboard no software keyboard appears. Recorded
    /// so host tests and the Simulator checks see the request.
    pub(crate) fn request_terminal_keyboard(&self, window: &Window, reason: &'static str) {
        window.request_virtual_keyboard();
        self.observation
            .record(serde_json::json!({"kind": "keyboard-request", "reason": reason}));
    }

    /// Gives the terminal its input destination, focus and text responder
    /// back, as a tap on it does. A chrome tap dismisses the keyboard and a
    /// switch detaches input; without this, typing after either went nowhere
    /// on a Mac until the terminal was clicked.
    pub(crate) fn restore_terminal_keys(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        reason: &'static str,
    ) {
        self.refresh_terminal_input();
        if self.terminal.open && self.terminal.input.ready {
            self.terminal_focus.focus(window, cx);
            self.request_terminal_keyboard(window, reason);
            self.request_text_retirement();
        }
        window.notify_text_input_changed(&self.terminal_focus);
    }

    /// Runs a restore a switch or a finished navigation left pending
    /// (`TerminalView::restore_keys`) once nothing else owns the keys.
    pub(crate) fn restore_pending_terminal_keys(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.terminal.restore_keys
            || self.terminal.navigating.is_some()
            || self.find.is_some()
            || self.input_modal()
            || self.actions.palette
            || self.shell.shortcuts.open
        {
            return;
        }
        self.terminal.restore_keys = false;
        self.restore_terminal_keys(window, cx, "switch-restore");
    }
    /// The status sits in the toolbar's wrapping row (terminal_view.rs): a
    /// longer text moves onto a second row and shrinks the terminal, and that
    /// resize clears a selection. PR 40's "Selection active · press a terminal
    /// key or turn Select off to type" did so the moment a selection began
    /// (SH1-152 Simulator check, CI run 44048410658: 407 to 373 px). Keep the
    /// P13 texts; an active discard fence shows its reason, not a historical
    /// composition event.
    pub(crate) fn terminal_status(&self) -> String {
        if self.terminal.navigating.is_some() {
            return "Navigating… · input paused".into();
        }
        if let Some(reason) = self.find_keyboard_status() {
            return reason.into();
        }
        if let Some(status) = &self.actions.key_status
            && status.current(&self.terminal, self.board.selection)
        {
            return status.text.clone();
        }
        // #270: a field that closed under the user holds typing until a tap.
        if self.terminal.live.connected
            && let Some(reason) = self.terminal.input.held_until_tap()
        {
            return format!("{reason} · tap terminal to type");
        }
        if self.terminal.live.connected && self.terminal.input.requires_gesture() {
            if let Some(reason) = self.retirement.reason {
                return reason.into();
            }
            #[cfg(target_os = "ios")]
            if let Some(status) = self.native_retirement_status() {
                return status.into();
            }
            return "Text input paused · tap terminal to type".into();
        }
        if self.terminal.live.status.is_empty() {
            return "Connecting…".into();
        }
        self.terminal.live.status.clone()
    }

    pub(crate) fn refresh_terminal_input(&mut self) {
        // A host switch must not encode keys with the new view's defaults.
        self.sync_input_settings();
        let modal = self.input_modal();
        let terminal = &mut self.terminal;
        if modal
            || self.find.is_some()
            || terminal.navigating.is_some()
            || self.actions.palette
            || self.shell.shortcuts.open
        {
            terminal.input.detach();
            return;
        }
        // A busy mailbox means the reader or the core worker is publishing.
        // Keep the last observed state, as sync_handoff does: input generated
        // now still leaves in order, and each send path rechecks the target
        // (Link::send_verified, the core worker's destination check). Marking
        // input unready here held it behind later requests (P13 D5) and made
        // start_paste refuse a paste during a publication (L14).
        if let Ok(live) = terminal.link.inbox.try_lock() {
            terminal
                .input
                .sync(&live, terminal.link.generation, terminal.size);
        }
    }
    pub(crate) fn send_terminal(&mut self, event: GeneratedInput) {
        if self.find.is_some() || self.find_handoff.paused {
            return;
        }
        self.refresh_terminal_input();
        self.sync_handoff();
        if self.handoff.blocks_input() {
            return;
        }
        if self.handoff.preparing {
            self.hold_for_paste(event);
            return;
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let receipt = serde_json::json!({"kind":"terminal-input", "variant":match &event.event {
            sheprd_client::protocol::ClientPaneInputEvent::Paste(_) => "Paste",
            sheprd_client::protocol::ClientPaneInputEvent::Key { .. } => "Key",
            sheprd_client::protocol::ClientPaneInputEvent::TextCommit(_) => "TextCommit",
            sheprd_client::protocol::ClientPaneInputEvent::Mouse { .. } => "Mouse",
        }, "terminal":match &event.destination.target { crate::terminal::InputTarget::Pane(id) | crate::terminal::InputTarget::Popup(id) => id },
        "mouse_kind":match &event.event { sheprd_client::protocol::ClientPaneInputEvent::Mouse { kind, .. } => Some(format!("{kind:?}")), _ => None },
        "lines":match &event.event { sheprd_client::protocol::ClientPaneInputEvent::Mouse { lines, .. } => Some(*lines), _ => None }});
        if !self.terminal.input.queue(event) {
            self.terminal.live.status = "Input refused: disconnected or queue full".into();
            return;
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        self.observation.record(receipt);
        self.flush_terminal();
    }
    pub(crate) fn flush_terminal(&mut self) {
        if self.find.is_some() || self.find_handoff.paused {
            return;
        }
        if self.input_modal()
            || self.terminal.navigating.is_some()
            || self.actions.palette
            || self.shell.shortcuts.open
        {
            self.terminal.input.detach();
            return;
        }
        while let Some(event) = self.terminal.input.next() {
            let Some(target) = &self.terminal.input.destination else {
                break;
            };
            let result = if let Some(core) = &self.terminal.core {
                core.send(crate::core::Command::Input(target.clone(), event))
            } else {
                self.terminal.link.send_verified(target, event)
            };
            if let Err(error) = result {
                self.terminal.live.status = error.to_string();
                self.terminal.input.detach(); // Preserve an actual discard on writer failure.
                break;
            }
        }
    }
    pub(crate) fn open_settings_from_shortcut(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shell.shortcuts.open = false;
        self.begin_settings_modal(window, cx);
    }

    pub(crate) fn terminal_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        crate::p11_observer::callback(
            &self.observation,
            if self.settings_modal {
                "terminal-modal"
            } else {
                "terminal"
            },
            "down",
        );
        if self.terminal_focus.is_focused(window) && is_settings_shortcut(&event.keystroke) {
            if !event.is_held {
                self.open_settings_from_shortcut(window, cx);
            }
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        // The modal (Settings or Hosts, A6) consumes every key before any
        // native edit routing below.
        if self.input_modal() {
            self.actions.host_keys.clear_press();
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            if std::env::var_os("YEBISU_P11_QA").is_some()
                && let Some(action) = herdr_uikit::native_edit_action()
            {
                self.observation
                    .record(serde_json::json!({"kind":"p11-native-blocked", "action":format!("{action:?}")}));
            }
            self.actions.consumed.insert(event.keystroke.key.clone());
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        // Search owns input while open, and the retirement fence holds every
        // terminal key, native Edit commands included, until it is released.
        if self.find.is_some() {
            self.actions.host_keys.clear_press();
            return;
        }
        // Every KeyDown of the press that closed search, held or not, stays
        // with the UI until that press ends (find/closing_press.rs).
        if self.closing_press_owns(event) {
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        if self.find_handoff.paused {
            if self.terminal_focus.is_focused(window) {
                cx.stop_propagation();
                window.prevent_default();
            }
            return;
        }
        // A held closing key is not a fresh gesture that can resume input.
        if self.terminal_focus.is_focused(window)
            && event.is_held
            && self.terminal.input.requires_gesture()
        {
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        self.input_transition(|this| this.terminal_key_down_inner(event, window, cx))
    }
    fn terminal_key_down_inner(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.terminal_focus.is_focused(window) {
            self.actions.host_keys.clear_press();
            return;
        }
        if !event.is_held {
            self.actions.key_status = None;
        }
        // Escape closes the Shortcuts popover, never reaching the host (S1).
        if self.shell.shortcuts.open && event.keystroke.key == "escape" {
            self.shell.shortcuts.open = false;
            self.actions.consumed.insert(event.keystroke.key.clone());
            cx.stop_propagation();
            window.prevent_default();
            window.notify_text_input_changed(&self.terminal_focus);
            cx.notify();
            return;
        }
        // Any other key, paste and copy included, stays in the app while
        // the popover is open.
        if self.shell.shortcuts.open {
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        #[cfg(target_os = "ios")]
        if let Some(action) = herdr_uikit::native_edit_action() {
            use crate::edit_menu::{EditAction, MenuToken};
            use herdr_uikit::NativeEditAction;
            #[cfg(target_abi = "sim")]
            if std::env::var_os("YEBISU_NATIVE_QA").is_some() {
                self.observation.record(serde_json::json!({
                    "kind":"native-consumer", "action":format!("{action:?}"), "modal":self.settings_modal
                }));
            }
            let token = herdr_uikit::native_edit_menu_token().map(MenuToken::Native);
            if token.is_some() || self.terminal_focus.is_focused(window) {
                let action = match action {
                    NativeEditAction::Copy => EditAction::Copy,
                    NativeEditAction::Cut => EditAction::Cut,
                    NativeEditAction::Paste => EditAction::Paste,
                    NativeEditAction::SelectAll => EditAction::SelectAll,
                };
                self.native_menu_action(action, token, window, cx);
                return;
            }
        }
        if !self.terminal_focus.is_focused(window) {
            return;
        }
        // Only repeats belong to the previous UI-owned press. A new press
        // recovers if its release was lost while another screen had focus.
        if !event.is_held {
            self.actions.consumed.remove(&event.keystroke.key);
        }
        if event.is_held && self.actions.consumed.contains(&event.keystroke.key) {
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        if self.actions.palette && event.keystroke.key == "escape" {
            self.actions.palette = false;
            self.actions.consumed.insert(event.keystroke.key.clone());
            cx.stop_propagation();
            window.prevent_default();
            cx.notify();
            return;
        }
        let prefix_context = self.herdr_prefix_context();
        if prefix_context {
            self.poll_host_keys(cx);
        }
        let route = self.actions.host_keys.route(
            &self.terminal,
            self.board.selection,
            prefix_context,
            self.actions.use_server_keybindings,
            event,
        );
        let command = match route {
            Route::Consume(None) => {
                self.actions.consumed.insert(event.keystroke.key.clone());
                cx.stop_propagation();
                window.prevent_default();
                return;
            }
            Route::Consume(command) => command,
            Route::Native if !self.actions.herdr_prefix => {
                crate::actions::shortcut_candidate(&self.terminal, &self.actions, &event.keystroke)
            }
            Route::Native | Route::Terminal | Route::Replay => None,
        };
        if !matches!(route, Route::Consume(_) | Route::Replay)
            && event.keystroke.modifiers.platform
            && (!self.actions.herdr_prefix || event.keystroke.modifiers.number_of_modifiers() == 1)
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.function
            && ((event.keystroke.key == "v" && !event.keystroke.modifiers.alt)
                || (event.keystroke.key == "c" && !event.keystroke.modifiers.shift))
            && !(self.actions.terminal_priority && self.terminal.input.options.command_control)
        {
            if !event.is_held {
                if event.keystroke.key == "c" {
                    if event.keystroke.modifiers.alt {
                        self.copy_selection_as_one_line(cx);
                    } else {
                        self.copy_selection(cx);
                    }
                } else {
                    self.start_paste(event.keystroke.modifiers.shift, cx);
                }
            }
            self.actions.consumed.insert(event.keystroke.key.clone());
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        if let Some(command) = command {
            let herdr = matches!(route, Route::Consume(_));
            if self.run_key_command(command, herdr, event.is_held, window, cx) {
                self.actions.consumed.insert(event.keystroke.key.clone());
            }
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        self.refresh_terminal_input();
        self.sync_handoff();
        // An old held key cannot end a new selection.
        if event.is_held && self.handoff.blocks_input() {
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        let input = self
            .terminal
            .input
            .key_down(event, self.terminal.input.report_all);
        let modifiers = event.keystroke.modifiers;
        let printable = !(modifiers.control || modifiers.platform || modifiers.function);
        // A fresh key ends Select. GPUI declines a printable key and UIKit
        // types it through insertText, so that text must not meet the
        // selection gate either (SH1-156).
        if !event.is_held && (input.is_some() || printable) {
            self.handoff.resume_typing();
        }
        if let Some(input) = input {
            self.send_terminal(input);
            cx.stop_propagation();
            window.prevent_default();
            window.notify_text_input_changed(&self.terminal_focus);
            cx.notify();
        }
    }
    fn herdr_prefix_context(&self) -> bool {
        self.actions.herdr_prefix
            && self.terminal.live.connected
            && !self.terminal.reconnect.suspended
            && !self.actions.palette
            && self.terminal.navigating.is_none()
            && crate::actions::catalog_context_available(&self.terminal)
    }

    /// Run a shortcut's command. Returns whether its press stays the app's
    /// until the release. `herdr` is a Herdr prefix style key binding.
    fn run_key_command(
        &mut self,
        command: sheprd_commands::Command,
        herdr: bool,
        held: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        use sheprd_commands::Command;
        match command {
            Command::Keybinds => {
                if !held {
                    self.toggle_shortcuts(window, cx);
                }
                true
            }
            Command::Find => {
                if !held {
                    self.open_find(window, cx);
                }
                false
            }
            _ => {
                if !held {
                    // A live update replaces the live status at once, so
                    // the reason stays visible until the next key (L57).
                    let failure = self.run_shortcut(command, herdr, window).err();
                    self.actions.key_status = failure.map(|error| {
                        crate::actions::KeyStatus::new(
                            format!("{}: {error}", command_name(command)),
                            &self.terminal,
                            self.board.selection,
                        )
                    });
                }
                window.notify_text_input_changed(&self.terminal_focus);
                cx.notify();
                true
            }
        }
    }

    /// A Herdr binding is not an iPad ⌘ shortcut. Terminal priority with
    /// Command as Control gives ⌘ keys to the terminal, so the OS menu rows
    /// are unavailable then (native_menus.rs `catalog_rows`); that must not
    /// refuse a Herdr chord, which holds no ⌘ (L57: ctrl+a c swallowed c and
    /// created no tab). The focus requirement still applies.
    fn run_shortcut(
        &mut self,
        command: sheprd_commands::Command,
        herdr: bool,
        window: &Window,
    ) -> crate::Result<()> {
        if !herdr {
            return self.run_catalog_command(command, None, window.is_window_active());
        }
        if !window.is_window_active() {
            return Err(crate::Error::NativeMenu("inactive terminal focus"));
        }
        self.run_action(command)
    }

    /// The prefix route of `terminal_key_down` for a key without a KeyDown
    /// of its own: the key band's keys and the text UIKit types, from the
    /// software keyboard or through an IME. Returns whether the app took the
    /// key; a replayed prefix and every other key are the caller's to send.
    pub(crate) fn route_generated_key(
        &mut self,
        keystroke: Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.terminal_focus.is_focused(window) {
            return false;
        }
        self.actions.key_status = None;
        let prefix_context = self.herdr_prefix_context();
        if prefix_context {
            self.poll_host_keys(cx);
        }
        let event = KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        };
        match self.actions.host_keys.route(
            &self.terminal,
            self.board.selection,
            prefix_context,
            self.actions.use_server_keybindings,
            &event,
        ) {
            Route::Consume(None) => true,
            Route::Consume(Some(command)) => {
                self.run_key_command(command, true, false, window, cx);
                true
            }
            Route::Native | Route::Terminal | Route::Replay => false,
        }
    }

    pub(crate) fn terminal_key_up(
        &mut self,
        event: &KeyUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        crate::p11_observer::callback(
            &self.observation,
            if self.settings_modal {
                "terminal-modal"
            } else {
                "terminal"
            },
            "up",
        );
        if self.end_closing_press(&event.keystroke.key) {
            cx.stop_propagation();
            return;
        }
        // SH1-156: the SDK refuses a retirement request while a key is held
        // (held-press ledger, herdr-uikit PR 16) and admits it again after the
        // release, so a release submits the pending request once more.
        if self.terminal.input.requires_gesture() && !self.input_modal() && self.find.is_none() {
            self.request_text_retirement();
        }
        if self.input_modal() {
            self.actions.consumed.remove(&event.keystroke.key);
            cx.stop_propagation();
            return;
        }
        #[cfg(target_os = "ios")]
        if herdr_uikit::native_edit_action().is_some() {
            cx.stop_propagation();
            return;
        }
        if self.actions.consumed.remove(&event.keystroke.key) {
            cx.stop_propagation();
            return;
        }
        if self.find_handoff.paused {
            cx.stop_propagation();
            return;
        }
        self.refresh_terminal_input();
        if let Some(release) = self.terminal.input.key_up(&event.keystroke.key) {
            self.send_terminal(release);
            cx.stop_propagation();
        }
    }
    pub(crate) fn input_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let options = &self.terminal.input.options;
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .px_4()
            .text_sm()
            .child(
                div()
                    .id("option-mode")
                    .p_2()
                    .bg(rgb(0x3b4252))
                    .child(if options.option_alt {
                        "Option: Alt"
                    } else {
                        "Option: Native"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        // The view may still hold a new host's defaults.
                        this.sync_input_settings();
                        this.terminal.input.options.option_alt ^= true;
                        this.toolbar_setting_changed(ToolbarSetting::InputOptions, cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("command-mode")
                    .p_2()
                    .bg(rgb(0x3b4252))
                    .child(if options.command_control {
                        "Command: Ctrl"
                    } else {
                        "Command: Local"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        // The view may still hold a new host's defaults.
                        this.sync_input_settings();
                        this.terminal.input.options.command_control ^= true;
                        this.toolbar_setting_changed(ToolbarSetting::InputOptions, cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("shortcut-priority")
                    .p_2()
                    .bg(rgb(0x3b4252))
                    .child(if self.actions.terminal_priority {
                        "Shortcuts: Terminal priority"
                    } else {
                        "Shortcuts: App priority"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.actions.terminal_priority ^= true;
                        this.toolbar_setting_changed(ToolbarSetting::InputOptions, cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("anchor-mode")
                    .p_2()
                    .bg(rgb(0x3b4252))
                    .child(if options.caret_anchor {
                        "Candidate: Caret"
                    } else {
                        "Candidate: Clause"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        // The view may still hold a new host's defaults.
                        this.sync_input_settings();
                        this.terminal.input.options.caret_anchor ^= true;
                        this.toolbar_setting_changed(ToolbarSetting::InputOptions, cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

impl Smoke {
    // Deterministic Simulator adapter exercise; never compiled into device UI.
    pub(crate) fn input_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if std::env::var_os("YEBISU_INPUT_QA").is_some() {
            use gpui::EntityInputHandler;
            return Some(
                div()
                    .flex()
                    .gap_4()
                    .px_4()
                    .child(
                        div()
                            .id("qa-mark")
                            .p_2()
                            .bg(rgb(0x3b4252))
                            .child("QA mark")
                            .children(crate::qa_ax::mark(crate::qa_ax::ids::BAND_MARK, "QA mark"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.terminal_focus.focus(window, cx);
                                this.replace_and_mark_text_in_range(
                                    None,
                                    "かな😀変換",
                                    Some(2..4),
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        div()
                            .id("qa-commit")
                            .p_2()
                            .bg(rgb(0x3b4252))
                            .child("QA commit")
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::BAND_COMMIT,
                                "QA commit",
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.replace_text_in_range(None, "かな😀変換", window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("qa-delete")
                            .p_2()
                            .bg(rgb(0x3b4252))
                            .child("QA delete")
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::BAND_DELETE,
                                "QA delete",
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let selected =
                                    this.selected_text_range(false, window, cx).map(|s| s.range);
                                this.replace_text_in_range(selected, "", window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("qa-cancel")
                            .p_2()
                            .bg(rgb(0x3b4252))
                            .child("QA cancel")
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::BAND_CANCEL,
                                "QA cancel",
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                // UIKit control_input's composing Esc callback.
                                let marked = this.marked_text_range(window, cx);
                                this.replace_text_in_range(marked, "", window, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        let _ = cx;
        None
    }
}

#[cfg(test)]
#[path = "terminal_controls_tests.rs"]
mod tests;

fn command_name(command: sheprd_commands::Command) -> String {
    sheprd_commands::COMMANDS
        .iter()
        .find(|info| info.command == command)
        .map_or_else(|| format!("{command:?}"), crate::i18n::command)
}
