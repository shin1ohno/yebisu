use crate::{app::Smoke, terminal_input::GeneratedInput};
use gpui::{AnyElement, Context, KeyDownEvent, KeyUpEvent, Window, div, prelude::*, rgb};

impl Smoke {
    /// Render the active refusal reason, not a historical composition event.
    pub(crate) fn terminal_status(&self) -> String {
        if !self.terminal.live.connected {
            return if self.terminal.live.status.is_empty() {
                "Connecting… · input paused".into()
            } else {
                self.terminal.live.status.clone()
            };
        }
        if self.terminal.navigating.is_some() {
            return "Navigating… · wait for the terminal, then type again".into();
        }
        if self.actions.palette {
            return "Commands open · close Commands to type".into();
        }
        if self.handoff.blocks_input() {
            return "Selection active · press a terminal key or turn Select off to type".into();
        }
        if self.terminal.input.destination.is_none() || !self.terminal.input.ready {
            return "Waiting for terminal frame · input paused".into();
        }
        if self.terminal.input.requires_gesture() {
            return self
                .retirement
                .reason
                .unwrap_or("Text input paused · native retirement required")
                .into();
        }
        #[cfg(target_os = "ios")]
        if let Some(status) = self.native_retirement_status() {
            return status.into();
        }
        self.terminal.live.status.clone()
    }

    pub(crate) fn refresh_terminal_input(&mut self) {
        let terminal = &mut self.terminal;
        if terminal.navigating.is_some() || self.actions.palette {
            terminal.input.detach();
            return;
        }
        if let Ok(live) = terminal.link.inbox.try_lock() {
            terminal
                .input
                .sync(&live, terminal.link.generation, terminal.size);
        } else {
            terminal.input.ready = false;
        }
    }
    pub(crate) fn send_terminal(&mut self, event: GeneratedInput) {
        self.refresh_terminal_input();
        self.sync_handoff();
        if self.handoff.blocks_input() {
            return;
        }
        if self.handoff.preparing {
            self.hold_for_paste(event);
            return;
        }
        if !self.terminal.input.queue(event) {
            self.terminal.live.status = "Input refused: disconnected or queue full".into();
            return;
        }
        self.flush_terminal();
    }
    pub(crate) fn flush_terminal(&mut self) {
        if self.terminal.navigating.is_some() || self.actions.palette {
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
    pub(crate) fn terminal_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.terminal_focus.is_focused(window) {
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
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "retirement-probe"))]
        if std::env::var_os("YEBISU_RETIREMENT_QA").is_some() && event.keystroke.key == "f18" {
            // Probe through the real GPUI callback slot, with the SDK guard.
            // This reserved fixture key is not proof of physical key delivery.
            self.toggle_selection();
            self.toggle_selection();
            cx.notify();
            return;
        }
        if !self.terminal_focus.is_focused(window) {
            return;
        }
        #[cfg(target_os = "ios")]
        if let Some(action) = herdr_uikit::native_edit_action() {
            use herdr_uikit::NativeEditAction;
            match action {
                NativeEditAction::Paste => self.start_paste(false, cx),
                NativeEditAction::Copy | NativeEditAction::Cut => self.copy_selection(cx),
                NativeEditAction::SelectAll => {}
            }
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        if event.keystroke.modifiers.platform
            && !event.keystroke.modifiers.alt
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.function
            && (event.keystroke.key == "v"
                || (event.keystroke.key == "c" && !event.keystroke.modifiers.shift))
            && !(self.actions.terminal_priority && self.terminal.input.options.command_control)
        {
            if !event.is_held {
                if event.keystroke.key == "c" {
                    self.copy_selection(cx);
                } else {
                    self.start_paste(event.keystroke.modifiers.shift, cx);
                }
            }
            self.actions.consumed.insert(event.keystroke.key.clone());
            cx.stop_propagation();
            window.prevent_default();
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
        if let Some(command) =
            crate::actions::shortcut(&self.terminal, &self.actions, &event.keystroke)
        {
            if !event.is_held
                && let Err(e) = self.run_action(command)
            {
                self.terminal.live.status = e.to_string();
            }
            self.actions.consumed.insert(event.keystroke.key.clone());
            cx.stop_propagation();
            window.prevent_default();
            window.notify_text_input_changed(&self.terminal_focus);
            cx.notify();
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
        if let Some(input) = self
            .terminal
            .input
            .key_down(event, self.terminal.input.report_all)
        {
            if !event.is_held {
                self.handoff.resume_typing();
            }
            self.send_terminal(input);
            cx.stop_propagation();
            window.prevent_default();
            window.notify_text_input_changed(&self.terminal_focus);
            cx.notify();
        }
    }
    pub(crate) fn terminal_key_up(
        &mut self,
        event: &KeyUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(target_os = "ios")]
        if herdr_uikit::native_edit_action().is_some() {
            cx.stop_propagation();
            return;
        }
        if self.actions.consumed.remove(&event.keystroke.key) {
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
                        this.terminal.input.options.option_alt ^= true;
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
                        this.terminal.input.options.command_control ^= true;
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
                        this.terminal.input.options.caret_anchor ^= true;
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
        if std::env::var_os("YEBISU_R6_QA").is_some() {
            return Some(
                div()
                    .flex()
                    .gap_4()
                    .px_4()
                    .children(
                        [
                            ("r6-next", "QA Next", "}"),
                            ("r6-prev", "QA Previous", "{"),
                            ("r6-escape", "QA Escape", "escape"),
                        ]
                        .into_iter()
                        .map(|(id, label, key)| {
                            div()
                                .id(id)
                                .p_2()
                                .bg(rgb(0x3b4252))
                                .child(label)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.terminal_focus.focus(window, cx);
                                    if key == "escape" {
                                        this.actions.palette = true;
                                    }
                                    let keystroke = gpui::Keystroke {
                                        key: key.into(),
                                        key_char: None,
                                        modifiers: gpui::Modifiers {
                                            platform: key != "escape",
                                            ..Default::default()
                                        },
                                    };
                                    for is_held in [false, true, true, true] {
                                        this.terminal_key_down(
                                            &KeyDownEvent {
                                                keystroke: keystroke.clone(),
                                                is_held,
                                                prefer_character_input: false,
                                            },
                                            window,
                                            cx,
                                        );
                                    }
                                    this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
                                    this.observation.record(serde_json::json!({
                                        "kind":"r6-key", "key":key, "palette":this.actions.palette,
                                        "consumed":this.actions.consumed.len(),
                                    }));
                                    cx.notify();
                                }))
                        }),
                    )
                    .into_any_element(),
            );
        }
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
