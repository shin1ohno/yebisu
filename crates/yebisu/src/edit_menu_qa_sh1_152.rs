//! Source-defined Simulator cases; the driver checks wire and clipboard receipts.
use crate::{app::Smoke, edit_menu::MenuToken, edit_menu_qa_native as native, handoff::Selection};
use gpui::{AnyElement, ClipboardItem, Context, MouseButton, Window, div, prelude::*, px, rgb};
use std::{rc::Rc, sync::Arc, time::Duration};

#[derive(Default)]
pub(crate) struct MenuQa {
    pub capture: Option<Rc<native::Capture>>,
    pub token: Option<herdr_uikit::NativeEditMenuToken>,
    pub stage: usize,
    /// The E5 scene opened by "Second"; its controls carry a distinct label.
    pub second: bool,
    /// The stage whose Begin ran. The control then reads "Begun", so the
    /// driver can see a lost tap and retry without a second start marker.
    pub begun: Option<usize>,
    /// Whether the automatic E1-text fixture has closed its peer interval.
    e1_text_ended: bool,
}

/// The text a control paints, which is also its accessibility label. The
/// second E5 scene's labels never contain the first scene's ("152 Begin" is
/// not in "152 Two Begin"), and Begin reads "Begun" once its stage began, so
/// a lost tap stays visible on a screenshot.
fn label(second: bool, command: &str, begun: bool) -> &'static str {
    match (second, command, begun) {
        (false, "Begin", false) => "152 Begin",
        (false, "Begin", true) => "152 Begun",
        (false, "Select", _) => "152 Select",
        (false, "Mutate", _) => "152 Mutate",
        (false, "Replay", _) => "152 Replay",
        (false, "Fresh", _) => "152 Fresh",
        (false, "End", _) => "152 End",
        (false, "Ordinary", _) => "152 Ordinary",
        (false, "Restore", _) => "152 Restore",
        (false, "Second", _) => "152 Second",
        (true, "Begin", false) => "152 Two Begin",
        (true, "Begin", true) => "152 Two Begun",
        (true, "Select", _) => "152 Two Select",
        (true, "End", _) => "152 Two End",
        (true, "Close", _) => "152 Two Close",
        _ => "152 ?",
    }
}

/// The QA mark of one control (UI test redesign S4): EditMenuQA presses
/// each control by this identifier instead of reading its label off the screen.
fn mark(second: bool, command: &str, label: &'static str) -> Option<AnyElement> {
    use crate::qa_ax::ids;
    match (second, command) {
        (false, "Begin") => crate::qa_ax::mark(ids::SH1_152_BEGIN, label),
        (false, "Select") => crate::qa_ax::mark(ids::SH1_152_SELECT, label),
        (false, "Mutate") => crate::qa_ax::mark(ids::SH1_152_MUTATE, label),
        (false, "Replay") => crate::qa_ax::mark(ids::SH1_152_REPLAY, label),
        (false, "Fresh") => crate::qa_ax::mark(ids::SH1_152_FRESH, label),
        (false, "End") => crate::qa_ax::mark(ids::SH1_152_END, label),
        (false, "Ordinary") => crate::qa_ax::mark(ids::SH1_152_ORDINARY, label),
        (false, "Restore") => crate::qa_ax::mark(ids::SH1_152_RESTORE, label),
        (false, "Second") => crate::qa_ax::mark(ids::SH1_152_SECOND, label),
        (true, "Begin") => crate::qa_ax::mark(ids::SH1_152_TWO_BEGIN, label),
        (true, "Select") => crate::qa_ax::mark(ids::SH1_152_TWO_SELECT, label),
        (true, "End") => crate::qa_ax::mark(ids::SH1_152_TWO_END, label),
        (true, "Close") => crate::qa_ax::mark(ids::SH1_152_TWO_CLOSE, label),
        _ => None,
    }
}

/// The QA label of the strip's empty space (`ids::SH1_152_EMPTY`); it paints nothing.
const EMPTY: &str = "152 empty space";

pub(crate) fn case() -> String {
    std::env::var("YEBISU_SH1_152_CASE").unwrap_or_default()
}

fn clipboard_text(case: &str, stage: usize) -> String {
    format!("sh1-152-clip-{case}-{stage}")
}

impl Smoke {
    pub(crate) fn sh1_152_advance_e1_text(&self, window: &mut Window, cx: &mut Context<Self>) {
        if std::env::var_os("YEBISU_SH1_152_AUTO_MARKERS").is_none()
            || case() != "E1-text"
            || !self.terminal.input.ready
            || !window.is_window_active()
            || self.sh1_152_e1_text_command().is_none()
        {
            return;
        }
        // BackBoard can cancel either marker tap before GPUI sees the click. Begin prepares the
        // clipboard once the terminal is ready, and End follows the actual native Paste receipt.
        cx.defer_in(window, |this, window, cx| {
            let Some(command) = this.sh1_152_e1_text_command() else {
                return;
            };
            if !window.is_window_active() {
                return;
            }
            this.refresh_terminal_input();
            this.sync_handoff();
            if this.terminal.input.ready && this.handoff.stamp.is_some() {
                this.sh1_152_command(command, window, cx);
                this.menu_qa.e1_text_ended = command == "End";
            }
        });
    }

    fn sh1_152_e1_text_command(&self) -> Option<&'static str> {
        if self.menu_qa.begun.is_none() {
            Some("Begin")
        } else if !self.menu_qa.e1_text_ended
            && self.menu_qa.capture.is_some()
            && crate::qa_ax::peer::has_paste(&clipboard_text("E1-text", self.menu_qa.stage))
        {
            Some("End")
        } else {
            None
        }
    }

    pub(crate) fn sh1_152_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_SH1_152_CASE")?;
        let commands: &[&'static str] = if self.menu_qa.second {
            &["Begin", "Select", "End", "Close"]
        } else if case().starts_with("E5") {
            &["Begin", "Select", "End", "Second"]
        } else {
            &[
                "Begin", "Select", "Mutate", "Replay", "Fresh", "End", "Ordinary", "Restore",
            ]
        };
        let begun = self.menu_qa.begun == Some(self.menu_qa.stage);
        let second = self.menu_qa.second;
        Some(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(commands.iter().map(|&command| {
                    let label = label(second, command, command == "Begin" && begun);
                    div()
                        .id(format!("sh1-152-{command}"))
                        .p_2()
                        .bg(rgb(0x3b4252))
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.sh1_152_command(command, window, cx)
                        }))
                        // Last, so the mark lies over the whole control.
                        .children(mark(second, command, label))
                }))
                // Empty space with no handler that stops the touch (UI test
                // redesign S6): EditMenuQA presses it after each launch until
                // GPUI counts the press, before the first real control.
                .child(
                    div()
                        .id("sh1-152-empty")
                        .w(px(64.))
                        .h(px(36.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .children(crate::qa_ax::mark(crate::qa_ax::ids::SH1_152_EMPTY, EMPTY)),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn sh1_152_capture(
        &mut self,
        token: MenuToken,
        handle: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if case().is_empty() || self.menu_qa.capture.is_some() {
            return;
        }
        #[cfg(not(test))]
        let MenuToken::Native(token) = token;
        #[cfg(test)]
        let token = match token {
            MenuToken::Native(token) => token,
            #[cfg(test)]
            MenuToken::Recorded(_) => return,
        };
        self.menu_qa.token = Some(token);
        let stage = self.menu_qa.stage;
        cx.spawn(async move |weak, cx| {
            cx.background_executor().timer(Duration::from_millis(150)).await;
            // This future runs on the main thread without a GPUI update borrow.
            let capture = native::capture(token).map(Rc::new);
            let replay = capture.as_ref().and_then(|capture| crate::edit_menu_qa_scenes::captured(capture.clone(), stage));
            let _ = weak.update(cx, |this, _| {
                this.observation.record(serde_json::json!({"kind":"sh1-152-capture", "case":case(), "stage":this.menu_qa.stage, "count":capture.as_ref().map(|c| c.commands.len()), "responder":capture.as_ref().and_then(|c| native::responder(&c.window)), "scene":capture.as_ref().and_then(|c| native::scene_id(&c.window)), "token":token.value(), "generation":this.terminal.link.generation, "destination":this.handoff.stamp.as_ref().map(|s| format!("{:?}",s.destination.target))}));
                crate::qa_ax::count("sh152_capture");
                if let Some(replay) = replay {
                    this.observation.record(serde_json::json!({"kind":"sh1-152-cross-scene-replay", "sent":replay}));
                }
                this.menu_qa.capture = capture;
                this.observation.record(serde_json::json!({"kind":"sh1-152-capture-status", "stage":stage, "status":format!("{:?}",herdr_uikit::native_edit_menu_status(handle,token))}));
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
        // qa.state: the stage of the last marker and one count per edge, so a
        // test waits for the marker it pressed for and then for that text at the peer.
        let stage = self.menu_qa.stage;
        crate::qa_ax::report("sh152_stage", || stage.into());
        crate::qa_ax::count(if edge == "start" {
            "sh152_start"
        } else {
            "sh152_end"
        });
    }

    fn sh1_152_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            "Second" => {
                crate::edit_menu_qa_scenes::open_second(window, self.menu_qa.stage + 1, cx);
            }
            "Close" => {
                let closed = crate::edit_menu_qa_scenes::close_first(cx);
                self.observation
                    .record(serde_json::json!({"kind":"sh1-152-close-other", "closed":closed}));
                crate::qa_ax::count("sh152_close_other");
            }
            "Begin" | "Fresh" => {
                if command == "Begin" && self.menu_qa.begun == Some(self.menu_qa.stage) {
                    self.observation.record(serde_json::json!({"kind":"sh1-152-begin-repeat", "case":case(), "stage":self.menu_qa.stage}));
                    crate::qa_ax::count("sh152_begin_repeat");
                    return;
                }
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
                cx.write_to_clipboard(ClipboardItem::new_string(clipboard_text(
                    &case(),
                    self.menu_qa.stage,
                )));
                self.sh1_152_marker("start");
                self.menu_qa.begun = Some(self.menu_qa.stage);
                cx.notify();
                if case().starts_with("E1") {
                    self.focus.focus(window, cx);
                }
            }
            "Select" => {
                self.sync_handoff();
                if let Some(surface) = &self.terminal.live.surface {
                    // The Herdr 2 peer's 日本語 line can start below row 2
                    // (run 43609891164 published a blank first row), so find it.
                    let frame = &surface.frame;
                    let row = if self.terminal.core_mode {
                        (0..frame.height)
                            .find(|&y| {
                                frame
                                    .cells
                                    .get(usize::from(y) * usize::from(frame.width))
                                    .is_some_and(|cell| cell.symbol == "日")
                            })
                            .unwrap_or(2)
                    } else {
                        2
                    };
                    self.handoff.selection = Some(Selection {
                        frame: Arc::new(surface.frame.clone()),
                        anchor: (if self.terminal.core_mode { 0 } else { 1 }, row),
                        end: (
                            if case().starts_with("E5") {
                                7
                            } else if self.terminal.core_mode {
                                5
                            } else {
                                6
                            },
                            row,
                        ),
                        dragging: false,
                    });
                }
                self.observation.record(serde_json::json!({"kind":"sh1-152-selection", "case":case(), "stage":self.menu_qa.stage, "text":self.handoff.selection.as_ref().and_then(Selection::text)}));
                crate::qa_ax::count("sh152_selection");
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
                    "generation":self.terminal.link.generation, "destination":self.handoff.stamp.as_ref().map(|s| format!("{:?}",s.destination.target)),
                    "status":self.menu_qa.token.map(|t| format!("{:?}", herdr_uikit::native_edit_menu_status(window.window_handle(),t))),
                    "responder":self.menu_qa.capture.as_ref().and_then(|c| native::responder(&c.window))}));
                self.sh1_152_marker("end");
            }
            "Replay" | "Ordinary" => {
                let capture = self.menu_qa.capture.clone();
                let ordinary = command == "Ordinary";
                // Separates a missed tap from a replay that never reported
                // (PR run 43633426435, E3f-key had no replay record).
                self.observation.record(serde_json::json!({"kind":"sh1-152-replay-requested", "case":case(), "stage":self.menu_qa.stage, "ordinary":ordinary, "capture":capture.is_some()}));
                cx.spawn(async move |weak, cx| {
                    let Some(capture) = capture else {
                        let _ = weak.update(cx, |this,_| this.observation.record(serde_json::json!({"kind":"sh1-152-probe-error","error":"missing capture"})));
                        return;
                    };
                    // Ordinary: the delegate's menu for an unowned configuration (as UIKit's
                    // own secondary-click menu has) must stay empty, and the untagged
                    // suggestions it was offered are replayed as ordinary commands.
                    let (commands, delegate_count) = if ordinary {
                        native::delegate(&capture, false).map_or((None, None), |(menu, suggestions)| (Some(suggestions), Some(menu.len())))
                    } else if case() == "E3i" {
                        (native::delegate(&capture, true).map(|(menu, _)| menu), None)
                    } else {
                        (Some(capture.commands.clone()), None)
                    };
                    let count = commands.as_ref().map(|c| c.len());
                    let sent = commands.as_ref().map(|c| native::replay(c));
                    let _ = weak.update(cx, |this,_| {
                        this.observation.record(serde_json::json!({"kind":"sh1-152-replay", "case":case(), "stage":this.menu_qa.stage,"ordinary":ordinary,"count":count,"delegate_count":delegate_count,"sent":sent}));
                        crate::qa_ax::count("sh152_replay");
                    });
                }).detach();
            }
            "Mutate" => self.sh1_152_mutate(window, cx),
            _ => {}
        }
        cx.notify();
    }

    fn sh1_152_mutate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match case().as_str() {
            "E3a" | "E3i" | "E3a-core" => {
                let result = if let Some(core) = &self.terminal.core {
                    self.terminal.input.detach();
                    core.send(crate::core::Command::Split)
                } else {
                    // The mock peer publishes a newly focused pane at its
                    // default 80x24, not at the client size a real Herdr 1
                    // keeps. Resend the size after the focus request so input
                    // becomes ready even when the viewport never changes
                    // (run 43616762580, E3i without a software keyboard).
                    let result = self.run_action(sheprd_commands::Command::FocusRight);
                    self.terminal.size = None;
                    result
                };
                self.observation.record(serde_json::json!({"kind":"sh1-152-destination-change", "accepted":result.is_ok()}));
                crate::qa_ax::count("sh152_destination_change");
            }
            "E3c" => {
                self.handoff.selection = None;
            }
            "E3b" => {
                let generation = self.terminal.link.reset();
                self.terminal
                    .link
                    .install(generation, crate::mock_peer::connect());
                // As a real reconnect does, forget the old connection's size so
                // the new one is resized and its input becomes ready again.
                self.terminal.size = None;
            }
            "E3f-text" | "E3f-key" => {
                let capture = self.menu_qa.capture.clone();
                cx.spawn(async move |weak, cx| {
                    let Some(capture) = capture else { return; };
                    let result = native::external_round_trip(&capture);
                    let _ = weak.update(cx, |this,_| {
                        this.observation.record(serde_json::json!({"kind":"sh1-152-external","case":case(),"result":result}));
                        crate::qa_ax::count("sh152_external");
                    });
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
                    let _ = weak.update(cx, |this,_| {
                        this.observation.record(serde_json::json!({"kind":"sh1-152-switch","dropped":dropped,"samples":samples,"status":this.menu_qa.token.map(|t| format!("{:?}",herdr_uikit::native_edit_menu_status(handle,t)))}));
                        crate::qa_ax::count("sh152_switch");
                    });
                }).detach();
            }
            _ => {}
        }
    }
}
