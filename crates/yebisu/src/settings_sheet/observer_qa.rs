//! Real native modal matrix. A click advances only after actual worker and peer receipts.
use crate::app::Smoke;
use gpui::{AnyElement, Context, prelude::*};
use herdr_uikit::{NativeEditAvailability, native_edit_observer as sdk};
use sheprd_client::protocol::ClientPaneInputEvent;
impl Smoke {
    pub(crate) fn settings_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P11_QA")?;
        let label = if self.observer.failed {
            "Observer failed"
        } else if self.observer.phase > 24 {
            "Observer matrix collected"
        } else {
            "Observer next"
        };
        Some(super::view::button("p11-observer",label,true)
            .on_click(cx.listener(|this,_,window,cx| {
                if !this.settings_modal || this.observer.failed || this.observer.phase > 24 { return; }
                if this.observer.phase > 0 {
                    if !this.observer.complete() || this.handoff.workers != 0 || this.handoff.preparing { return; }
                    if !this.observer.end_sent {
                        if !this.observer_marker("end",this.observer.phase) { this.observer.fail(); return; }
                        this.observer.end_sent = true;
                        return; // A subsequent click must see the actual receiver acknowledgment.
                    }
                    if !crate::mock_peer::observer_ack(this.observer.phase) { return; }
                    if !this.observation.try_record(serde_json::json!({"kind":"p11-peer-barrier","phase":this.observer.phase,"workers":this.handoff.workers,"preparing":this.handoff.preparing})) {
                        this.observer.fail(); return;
                    }
                }
                this.observer.phase += 1;
                this.observer.end_sent = false;
                if this.observer.phase > 24 {
                    this.observer.finish(&this.observation);
                    // No synthetic inactive/no-responder receipt: current API cannot force these states.
                    this.observation.record(serde_json::json!({"kind":"p11-unsupported","phases":["inactive","no-responder"],"full_acceptance":false}));
                    cx.notify(); return;
                }
                let i = this.observer.phase - 1;
                let action = [sdk::Action::Copy,sdk::Action::Paste,sdk::Action::Cut,sdk::Action::SelectAll][(i%4) as usize];
                let origin = if (i/4)%2 == 0 {sdk::ResponderOrigin::Text} else {sdk::ResponderOrigin::Drawing};
                let allow = i/8 == 2;
                let policy = if i/8 == 0 {None} else {Some(NativeEditAvailability{copy:allow,cut:allow,paste:allow,select_all:allow})};
                // Keep real sheet focus. Some/allow must reach the sheet's actual handler.
                this.settings_focus.focus(window,cx);
                let restore = this.native_edit_availability(window);
                if !this.observer_marker("start",this.observer.phase) { this.observer.fail(); return; }
                this.observer.start(window.window_handle(),&this.observation,action,origin,policy,restore);
                cx.notify();
            })).into_any_element())
    }
    fn observer_marker(&self, edge: &str, phase: u32) -> bool {
        self.terminal.link.handle.as_ref().is_some_and(|h| {
            h.send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste(format!(
                    "p11-observer-{edge}-{phase}"
                ))],
            )
            .is_ok()
        })
    }
}
