//! Simulator fixture only. Controls prepare state; only OS menu picks execute actions.
use crate::app::Smoke;
use gpui::{AnyElement, Context, KeyDownEvent, MouseButton, Window, div, prelude::*, px, rgb};
use sheprd_client::protocol::ClientPaneInputEvent;
use std::{
    sync::{Arc, mpsc::SyncSender},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct MenuQa {
    pub gate: Option<SyncSender<()>>,
    pub freeze: bool,
    pub deadline: Option<Instant>,
    case: &'static str,
    saved: Option<(String, Option<String>, Vec<String>)>,
    mutated: bool,
}
impl Smoke {
    pub(crate) fn p12_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P12_QA")?;
        Some(
            div()
                .id("p12-qa-controls")
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .absolute()
                .top(px(70.))
                .right(px(8.))
                .w(px(180.))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .bg(rgb(0x3b4252))
                .text_color(rgb(0xeceff4))
                .child(if self.terminal.navigating.is_some() {
                    "P12 Pending"
                } else {
                    "P12 Idle"
                })
                .children(
                    [
                        ("P12 Remote", "remote"),
                        ("P12 Sidebar", "sidebar"),
                        ("P12 Palette", "palette"),
                        ("P12 Release", "release"),
                        ("P12 Host", "host"),
                        ("P12 Pane", "pane"),
                        ("P12 Revision", "revision"),
                        ("P12 Focus", "focus"),
                        ("P12 Capability", "capability"),
                        ("P12 Reset", "reset"),
                        ("P12 Finish", "finish"),
                    ]
                    .into_iter()
                    .map(|(label, case)| {
                        div().id(label).p_1().child(label).on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.p12_prepare(case, window, cx);
                            },
                        ))
                    }),
                )
                .into_any_element(),
        )
    }
    fn p12_restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((host, pane, methods)) = self.p12_qa.saved.take() {
            // Restore only the injected field. Never resurrect connected state,
            // a stale snapshot, an inbox, a handle or a transport generation.
            match self.p12_qa.case {
                "host" => self.terminal.reconnect.target = host,
                "pane" => {
                    if let Some(snapshot) = self.terminal.live.snapshot.as_mut() {
                        Arc::make_mut(snapshot).focused_pane_id = pane;
                    }
                }
                "capability" => self.terminal.live.methods = methods,
                _ => {}
            }
        }
        self.p12_qa.freeze = false;
        self.p12_qa.deadline = None;
        self.terminal_focus.focus(window, cx);
    }
    fn p12_prepare(&mut self, case: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let before = [self.actions.board_hidden, self.actions.palette];
        self.observation
            .record(serde_json::json!({"kind":"p12-setup-begin", "step":case,
            "state":self.p12_state(window)}));
        if case == "reset" {
            self.observation.record(serde_json::json!({"kind":"p12-reset", "case":self.p12_qa.case,
                "sidebar":self.actions.board_hidden,"palette":self.actions.palette,"mutated":self.p12_qa.mutated}));
            self.p12_restore(window, cx);
        } else if case == "release" {
            let released = self
                .p12_qa
                .gate
                .take()
                .is_some_and(|gate| gate.try_send(()).is_ok());
            self.observation
                .record(serde_json::json!({"kind":"p12-release","released":released}));
        } else if case == "finish" {
            self.p12_restore(window, cx);
            // A real FIFO peer receipt, not a timer or local success flag.
            let result = self.terminal.link.handle.as_ref().map(|handle| {
                handle.send_input(
                    "boot-v1",
                    "w1:p1",
                    [ClientPaneInputEvent::Paste("p12-fence".into())],
                )
            });
            self.observation.record(
                serde_json::json!({"kind":"p12-finish","enqueued":matches!(result,Some(Ok(_))),
                    "reason":match &result { None => "no_handle", Some(Err(_)) => "send_error", Some(Ok(_)) => "queued" }}),
            );
        } else {
            self.p12_restore(window, cx);
            self.p12_qa.case = case;
            self.p12_qa.mutated = false;
            self.install_native_menus(window, cx);
            let negative = matches!(case, "host" | "pane" | "revision" | "focus" | "capability");
            if negative {
                self.p12_qa.saved = Some((
                    self.terminal.reconnect.target.clone(),
                    crate::board::snapshot(&self.terminal.live)
                        .and_then(|s| s.focused_pane_id.clone()),
                    self.terminal.live.methods.clone(),
                ));
                // Preserve an actually installed OS menu entry across injected scope change.
                self.p12_qa.freeze = true;
                self.p12_qa.deadline = Some(Instant::now() + Duration::from_secs(6));
            }
            self.observation.record(serde_json::json!({"kind":"p12-prepare","case":case,
                "pending":self.terminal.navigating.is_some(),"sidebar":self.actions.board_hidden,"palette":self.actions.palette,
                "negative":negative,"connected":self.terminal.live.connected,"state":self.p12_state(window),
                "catalog":crate::actions::catalog(&self.terminal).iter().map(|a|serde_json::json!({"name":a.info.name,"label":a.label,"enabled":a.available})).collect::<Vec<_>>()}));
        }
        self.observation
            .record(serde_json::json!({"kind":"p12-setup-end", "step":case,
            "before":before,"after":[self.actions.board_hidden,self.actions.palette],
            "state":self.p12_state(window)}));
        cx.notify();
    }
    pub(crate) fn p12_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.p12_qa.deadline.is_some_and(|d| Instant::now() >= d) {
            return;
        }
        self.p12_qa.deadline = None;
        match self.p12_qa.case {
            "host" => self.terminal.reconnect.target = "p12-other-host".into(),
            "pane" => {
                if let Some(snapshot) = self.terminal.live.snapshot.as_mut() {
                    Arc::make_mut(snapshot).focused_pane_id = Some("p12-other-pane".into());
                }
            }
            "revision" => self.p12_invalidate_revision(),
            "focus" => self.focus.focus(window, cx),
            "capability" => self.terminal.live.methods.retain(|m| m != "pane.split"),
            _ => return,
        }
        self.p12_qa.mutated = true;
        self.observation.record(serde_json::json!({"kind":"p12-mutated","case":self.p12_qa.case,
            "state":self.p12_state(window),"focused":self.terminal_focus.is_focused(window),"sidebar":self.actions.board_hidden,"palette":self.actions.palette}));
        cx.notify();
    }
    pub(crate) fn p12_state(&self, window: &Window) -> serde_json::Value {
        let snapshot = crate::board::snapshot(&self.terminal.live);
        serde_json::json!({
            "revision":self.p12_revision(), "scene_active":window.is_window_active(),
            "focused":self.terminal_focus.is_focused(window),
            "pending":self.terminal.navigating.is_some(), "connected":self.terminal.live.connected,
            "inbox_connected":self.terminal.link.inbox.try_lock().ok().map(|s|s.connected),
            "handle_present":self.terminal.link.handle.is_some(),
            "generation":self.terminal.link.generation,
            // Fixture host/pane identity is expressed as equality, not arbitrary user data.
            "original_host":self.terminal.reconnect.target != "p12-other-host",
            "original_pane":snapshot.and_then(|s|s.focused_pane_id.as_deref()) != Some("p12-other-pane"),
            "split_capability":self.terminal.live.methods.iter().any(|m|m == "pane.split"),
            "sidebar":self.actions.board_hidden,"palette":self.actions.palette,
        })
    }
    pub(crate) fn p12_menu_begin(
        &self,
        name: &str,
        captured_revision: Option<u64>,
        origin: &'static str,
        window: &Window,
    ) -> serde_json::Value {
        let receipt = serde_json::json!({"kind":"p12-menu-entry","case":self.p12_qa.case,
            "name":name,"origin":origin,"captured_revision":captured_revision,
            "current_revision":self.p12_revision(),"state":self.p12_state(window),
            "mutated":self.p12_qa.mutated,"before":[self.actions.board_hidden,self.actions.palette]});
        if std::env::var_os("YEBISU_P12_QA").is_some() {
            self.observation.record(receipt.clone());
        }
        receipt
    }
    pub(crate) fn p12_shortcut_entry(
        &self,
        event: &KeyDownEvent,
        window: &Window,
    ) -> Option<serde_json::Value> {
        std::env::var_os("YEBISU_P12_QA")?;
        let command =
            crate::actions::shortcut_candidate(&self.terminal, &self.actions, &event.keystroke)?;
        let name = crate::actions::catalog(&self.terminal)
            .into_iter()
            .find(|a| a.info.command == command)?
            .info
            .name;
        // SDK key_down carries no captured menu action identity. Never relabel
        // a key callback as native, or substitute current revision as capture.
        Some(self.p12_menu_begin(name, None, "shortcut_key_down", window))
    }
    pub(crate) fn p12_menu_end(
        &self,
        receipt: Option<serde_json::Value>,
        accepted: bool,
        guard: &'static str,
    ) {
        if std::env::var_os("YEBISU_P12_QA").is_none() {
            return;
        }
        let Some(mut receipt) = receipt else {
            return;
        };
        receipt["kind"] = "p12-menu-receiver".into();
        receipt["accepted"] = accepted.into();
        receipt["guard"] = guard.into();
        receipt["pending"] = self.terminal.navigating.is_some().into();
        receipt["after"] = serde_json::json!([self.actions.board_hidden, self.actions.palette]);
        self.observation.record(receipt);
    }
}
