//! Source-defined Simulator cases; the driver checks wire and clipboard receipts.
use crate::{app::Smoke, edit_menu::MenuToken, edit_menu_qa_native as native, handoff::Selection};
use gpui::{AnyElement, ClipboardItem, Context, Window, div, prelude::*, rgb};
use std::{rc::Rc, sync::Arc, time::Duration};

#[derive(Default)]
pub(crate) struct MenuQa {
    pub capture: Option<Rc<native::Capture>>,
    pub token: Option<herdr_uikit::NativeEditMenuToken>,
    pub stage: usize,
}

pub(crate) fn case() -> String {
    std::env::var("YEBISU_SH1_152_CASE").unwrap_or_default()
}

impl Smoke {
    pub(crate) fn sh1_152_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_SH1_152_CASE")?;
        Some(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(
                    [
                        "Begin", "Select", "Mutate", "Replay", "Fresh", "End", "Ordinary",
                        "Restore",
                    ]
                    .map(|command| {
                        div()
                            .id(format!("sh1-152-{command}"))
                            .p_2()
                            .bg(rgb(0x3b4252))
                            .child(format!("152 {command}"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.sh1_152_command(command, window, cx)
                            }))
                    }),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn sh1_152_capture(&mut self, token: MenuToken, cx: &mut Context<Self>) {
        if case().is_empty() || self.menu_qa.capture.is_some() {
            return;
        }
        let token = match token {
            MenuToken::Native(token) => token,
            #[cfg(test)]
            MenuToken::Recorded(_) => return,
        };
        self.menu_qa.token = Some(token);
        cx.spawn(async move |weak, cx| {
            cx.background_executor().timer(Duration::from_millis(150)).await;
            // This future runs on the main thread without a GPUI update borrow.
            let capture = native::capture(token).map(Rc::new);
            let _ = weak.update(cx, |this, _| {
                this.observation.record(serde_json::json!({"kind":"sh1-152-capture", "case":case(), "stage":this.menu_qa.stage, "count":capture.as_ref().map(|c| c.commands.len()), "responder":capture.as_ref().and_then(|c| native::responder(&c.window)), "token":token.value()}));
                this.menu_qa.capture = capture;
            });
        }).detach();
    }

    fn sh1_152_marker(&mut self, edge: &str) {
        self.refresh_terminal_input();
        self.sync_handoff();
        let Some(destination) = self.handoff.stamp.as_ref().map(|s| s.destination.clone()) else {
            return;
        };
        let text = format!("sh1-152-{edge}-{}-{}", case(), self.menu_qa.stage);
        let event = sheprd_client::protocol::ClientPaneInputEvent::Paste(text);
        let result = if let Some(core) = &self.terminal.core {
            core.send(crate::core::Command::Input(destination, event))
        } else {
            self.terminal.link.send_verified(&destination, event)
        };
        self.observation.record(serde_json::json!({"kind":"sh1-152-marker", "edge":edge, "case":case(), "stage":self.menu_qa.stage,"sent":result.is_ok()}));
    }

    fn sh1_152_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            "Begin" | "Fresh" => {
                if command == "Fresh" {
                    self.menu_qa.stage += 1;
                    self.menu_qa.capture = None;
                    self.menu_qa.token = None;
                }
                self.smoke_mode = false;
                self.actions.palette = false;
                self.handoff.selection = None;
                self.handoff.select_mode = false;
                self.handoff.drag_select = false;
                self.terminal_focus.focus(window, cx);
                if case().contains("key") {
                    window.dismiss_virtual_keyboard();
                } else {
                    window.request_virtual_keyboard();
                }
                cx.write_to_clipboard(ClipboardItem::new_string(format!(
                    "sh1-152-clip-{}-{}",
                    case(),
                    self.menu_qa.stage
                )));
                self.sh1_152_marker("start");
                if case().starts_with("E1") {
                    self.focus.focus(window, cx);
                }
            }
            "Select" => {
                self.sync_handoff();
                if let Some(surface) = &self.terminal.live.surface {
                    self.handoff.selection = Some(Selection {
                        frame: Arc::new(surface.frame.clone()),
                        anchor: (if self.terminal.core_mode { 0 } else { 1 }, 2),
                        end: (if self.terminal.core_mode { 5 } else { 6 }, 2),
                        dragging: false,
                    });
                }
                self.observation.record(serde_json::json!({"kind":"sh1-152-selection", "case":case(), "stage":self.menu_qa.stage, "text":self.handoff.selection.as_ref().and_then(Selection::text)}));
            }
            "Restore" => {
                self.terminal_focus.focus(window, cx);
                if case().contains("key") {
                    window.dismiss_virtual_keyboard();
                } else {
                    window.request_virtual_keyboard();
                }
            }
            "End" => {
                self.observation.record(serde_json::json!({"kind":"sh1-152-result", "case":case(), "stage":self.menu_qa.stage,
                    "clipboard":cx.read_from_clipboard().and_then(|c| c.text()),
                    "status":self.menu_qa.token.map(|t| format!("{:?}", herdr_uikit::native_edit_menu_status(window.window_handle(),t))),
                    "responder":self.menu_qa.capture.as_ref().and_then(|c| native::responder(&c.window))}));
                self.sh1_152_marker("end");
            }
            "Replay" | "Ordinary" => {
                let capture = self.menu_qa.capture.clone();
                let ordinary = command == "Ordinary";
                cx.spawn(async move |weak, cx| {
                    let Some(capture) = capture else {
                        let _ = weak.update(cx, |this,_| this.observation.record(serde_json::json!({"kind":"sh1-152-probe-error","error":"missing capture"})));
                        return;
                    };
                    let commands = if case() == "E3i" || ordinary { native::delegate(&capture, !ordinary) } else { Some(capture.commands.clone()) };
                    let count = commands.as_ref().map(|c| c.len());
                    let sent = commands.as_ref().map(|c| native::replay(c));
                    let _ = weak.update(cx, |this,_| this.observation.record(serde_json::json!({"kind":"sh1-152-replay", "case":case(), "stage":this.menu_qa.stage,"ordinary":ordinary,"count":count,"sent":sent})));
                }).detach();
            }
            "Mutate" => self.sh1_152_mutate(window, cx),
            _ => {}
        }
        cx.notify();
    }

    fn sh1_152_mutate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match case().as_str() {
            "E3a" | "E3i" => {
                let result = if let Some(core) = &self.terminal.core {
                    self.terminal.input.detach();
                    core.send(crate::core::Command::Split)
                } else {
                    self.run_action(sheprd_commands::Command::FocusRight)
                };
                self.observation.record(serde_json::json!({"kind":"sh1-152-destination-change", "accepted":result.is_ok()}));
            }
            "E3c" => {
                self.handoff.selection = None;
            }
            "E3b" => {
                let generation = self.terminal.link.reset();
                self.terminal
                    .link
                    .install(generation, crate::mock_peer::connect());
            }
            "E3f-text" | "E3f-key" => {
                let capture = self.menu_qa.capture.clone();
                cx.spawn(async move |weak, cx| {
                    let Some(capture) = capture else { return; };
                    let result = native::external_round_trip(&capture);
                    let _ = weak.update(cx, |this,_| this.observation.record(serde_json::json!({"kind":"sh1-152-external","case":case(),"result":result})));
                }).detach();
            }
            "E3g-text" => window.dismiss_virtual_keyboard(),
            "E3g-key" => window.request_virtual_keyboard(),
            "E3h" => {
                let handle = window.window_handle();
                let _ = herdr_uikit::take_native_edit_menu_switch_observations(handle);
                window.request_virtual_keyboard();
                cx.spawn(async move |weak,cx| {
                    cx.background_executor().timer(Duration::from_millis(300)).await;
                    let (dropped, samples) = herdr_uikit::take_native_edit_menu_switch_observations(handle);
                    let samples: Vec<_> = samples.into_iter().map(|s| serde_json::json!({"restart_reached":s.restart_reached,"acquire_returns":s.acquire_returns,"drawing_owned":s.drawing_owned,"text_owned":s.text_owned})).collect();
                    let _ = weak.update(cx, |this,_| this.observation.record(serde_json::json!({"kind":"sh1-152-switch","dropped":dropped,"samples":samples,"status":this.menu_qa.token.map(|t| format!("{:?}",herdr_uikit::native_edit_menu_status(handle,t)))})));
                }).detach();
            }
            _ => {}
        }
    }
}
