//! Opt-in real app flow. No authorization injection, permission request or audio proof.
use super::*;
use crate::settings_sheet::button;
use gpui::canvas;
use sheprd_client::protocol::ClientPaneInputEvent;

impl Smoke {
    pub(super) fn p14_controls(&self, cx: &Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P14_QA")?;
        Some(
            button("p14-next", "P14 next", self.notifications.qa_step < 5)
                .on_click(cx.listener(|this, _, _, cx| this.p14_next(cx)))
                .into_any_element(),
        )
    }
    fn p14_next(&mut self, cx: &mut Context<Self>) {
        if self.notifications.qa_step >= 5 || self.terminal.reconnect.target != "qa@private-peer" {
            return;
        }
        self.notifications.qa_step += 1;
        let step = self.notifications.qa_step;
        let mut policy = Policy {
            route: Route::Toast,
            sound: false,
            ..Policy::default()
        };
        if step == 2 {
            let local = chrono::Local::now();
            let minute = (local.hour() * 60 + local.minute()) as u16;
            policy.quiet = Some((minute, (minute + 60) % 1440));
        }
        self.notifications.set_policy(policy);
        let command = match step {
            1 => "p14-live",
            2 => "p14-quiet",
            3 => "p14-invalid",
            4 => "p14-disconnect",
            _ => "p14-stale-tap",
        };
        let before = self.terminal.link.generation;
        let sent = if step == 5 {
            let tag = self.notifications.qa_last_tag.clone().unwrap_or_default();
            self.open_notification(&tag, false, cx);
            false
        } else {
            self.terminal.link.handle.as_ref().is_some_and(|handle| {
                handle
                    .send_input(
                        "boot-v1",
                        "w1:p1",
                        [ClientPaneInputEvent::Paste(command.into())],
                    )
                    .is_ok()
            })
        };
        self.observation.record(serde_json::json!({"kind":"p14-step","step":step,"command":command,"sent":sent,"generation_before":before,"generation_after":self.terminal.link.generation,"has_handle":self.terminal.link.handle.is_some(),"status":self.notifications.status}));
        cx.notify();
    }
    pub(super) fn p14_paint(&self, cx: &Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P14_QA")?;
        let view = cx.entity();
        Some(canvas(|_,_,_| (), move |bounds,_,_,cx| {
            let this = view.read(cx);
            let state = &this.notifications;
            let snapshot = this.terminal.live.snapshot.as_ref();
            this.observation.record(serde_json::json!({"kind":"p14-paint","step":state.qa_step,
                "toast":state.toast.as_ref().map(|t|t.title),"tag":state.toast.as_ref().map(|t|&t.tag),
                "bounds":[f32::from(bounds.origin.x),f32::from(bounds.origin.y),f32::from(bounds.size.width),f32::from(bounds.size.height)],
                "host":this.terminal.reconnect.target,"generation":this.terminal.link.generation,
                "boot":snapshot.map(|s|&s.boot_id),"pane":snapshot.and_then(|s|s.focused_pane_id.as_ref()),
                "connected":this.terminal.live.connected,"active":this.terminal.link.notification_active.load(std::sync::atomic::Ordering::Acquire),
                "authorization":format!("{:?}",state.authorization),"status":state.status}));
        }).absolute().top_0().left_0().size_full().into_any_element())
    }
}
