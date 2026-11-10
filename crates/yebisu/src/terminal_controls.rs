use crate::{
    app::Smoke,
    terminal_input::{GeneratedInput, TerminalInput},
};
use gpui::{AnyElement, Context, KeyDownEvent, KeyUpEvent, Window, div, prelude::*, rgb};

impl Smoke {
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
        if self.handoff.selection.is_some() || self.handoff.select_mode {
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
            if let Err(error) = self.terminal.link.send_verified(target, event) {
                self.terminal.live.status = error.to_string();
                let options = std::mem::take(&mut self.terminal.input.options);
                self.terminal.input = TerminalInput::default();
                self.terminal.input.options = options;
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
            && !(self.actions.terminal_priority && self.terminal.input.options.command_control)
            && matches!(event.keystroke.key.as_str(), "c" | "v")
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
        if let Some(input) = self
            .terminal
            .input
            .key_down(event, self.terminal.input.report_all)
        {
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
                                this.terminal.input.fresh_gesture();
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
