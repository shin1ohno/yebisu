//! Simulator fixture only. Controls prepare state; only OS menu picks execute actions.
use crate::{app::Smoke, live::Live};
use gpui::{AnyElement, Context, Window, div, prelude::*, px, rgb};
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
    saved: Option<(Live, String, u64)>,
    mutated: bool,
}
impl Smoke {
    pub(crate) fn p12_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P12_QA")?;
        Some(
            div()
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
                            move |this, _, window, cx| this.p12_prepare(case, window, cx),
                        ))
                    }),
                )
                .into_any_element(),
        )
    }
    fn p12_restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((live, host, generation)) = self.p12_qa.saved.take() {
            self.terminal.live = live;
            self.terminal.reconnect.target = host;
            self.terminal.link.generation = generation;
        }
        self.p12_qa.freeze = false;
        self.p12_qa.deadline = None;
        self.terminal_focus.focus(window, cx);
    }
    fn p12_prepare(&mut self, case: &'static str, window: &mut Window, cx: &mut Context<Self>) {
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
                serde_json::json!({"kind":"p12-finish","enqueued":matches!(result,Some(Ok(_)))}),
            );
        } else {
            self.p12_restore(window, cx);
            self.p12_qa.case = case;
            self.p12_qa.mutated = false;
            self.install_native_menus(window, cx);
            let negative = matches!(case, "host" | "pane" | "revision" | "focus" | "capability");
            if negative {
                self.p12_qa.saved = Some((
                    self.terminal.live.clone(),
                    self.terminal.reconnect.target.clone(),
                    self.terminal.link.generation,
                ));
                // Preserve an actually installed OS menu entry across injected scope change.
                self.p12_qa.freeze = true;
                self.p12_qa.deadline = Some(Instant::now() + Duration::from_secs(6));
            }
            self.observation.record(serde_json::json!({"kind":"p12-prepare","case":case,
                "pending":self.terminal.navigating.is_some(),"sidebar":self.actions.board_hidden,"palette":self.actions.palette,
                "negative":negative,"connected":self.terminal.live.connected,
                "catalog":crate::actions::catalog(&self.terminal).iter().map(|a|serde_json::json!({"name":a.info.name,"label":a.label,"enabled":a.available})).collect::<Vec<_>>()}));
        }
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
            "focused":self.terminal_focus.is_focused(window),"sidebar":self.actions.board_hidden,"palette":self.actions.palette}));
        cx.notify();
    }
    pub(crate) fn p12_menu_receipt(
        &self,
        name: &str,
        revision: Option<u64>,
        accepted: bool,
        before: (bool, bool),
    ) {
        if std::env::var_os("YEBISU_P12_QA").is_none() {
            return;
        }
        self.observation.record(
            serde_json::json!({"kind":"p12-menu-receiver","case":self.p12_qa.case,
            "name":name,"revision":revision,"accepted":accepted,"mutated":self.p12_qa.mutated,
            "pending":self.terminal.navigating.is_some(),"before":[before.0,before.1],
            "after":[self.actions.board_hidden,self.actions.palette]}),
        );
    }
}
