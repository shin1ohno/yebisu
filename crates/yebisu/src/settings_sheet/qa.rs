//! Simulator-only probes of production callbacks. Never a live key/host provider.
use crate::app::Smoke;
use gpui::{
    AnyElement, Context, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, prelude::*,
};
use sheprd_client::protocol::ClientPaneInputEvent;
use std::time::Duration;
impl Smoke {
    pub(crate) fn settings_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P11_QA")?;
        Some(super::view::button("p11-probe","Probe modal callbacks",true).on_click(cx.listener(|this,_,window,cx|{
            // Pinned SDK has no entry/refusal observer. Scheduling is not dispatch.
            this.observation.record(serde_json::json!({"kind":"p11-sdk-observation", "supported":false, "reason":"pinned-sdk-no-refusal-hook"}));
            this.p11_marker("p11-start");
            this.terminal_focus.focus(window,cx);
            this.replace_and_mark_text_in_range(None,"P11_LEAK_MARK",Some(0..1),window,cx);
            this.unmark_text(window,cx);
            this.replace_text_in_range(None,"P11_LEAK_COMMIT",window,cx);
            let key=Keystroke{key:"enter".into(),key_char:None,modifiers:Default::default()};
            for is_held in [false,true]{this.terminal_key_down(&KeyDownEvent{keystroke:key.clone(),is_held,prefer_character_input:false},window,cx);}
            this.terminal_key_up(&KeyUpEvent{keystroke:key},window,cx);
            this.start_paste(false,cx);this.copy_selection(cx);
            let action_blocked=this.run_action(sheprd_commands::Command::ToggleSidebar).is_err();
            let scheduled=["copy:","paste:","cut:","selectAll:"].into_iter().all(crate::native_edit_qa::schedule_selector);
            this.observation.record(serde_json::json!({"kind":"p11-probe-start","scheduled":scheduled,"action_blocked":action_blocked,"modal":this.settings_modal}));
            cx.spawn(async move|weak,cx|{
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _=weak.update(cx,|this,cx|{this.p11_marker("p11-end");this.observation.record(serde_json::json!({"kind":"p11-probe-end","modal":this.settings_modal}));cx.notify();});
            }).detach();
        })).into_any_element())
    }
    fn p11_marker(&self, label: &str) {
        if let Some(handle) = &self.terminal.link.handle {
            let _ = handle.send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste(label.into())],
            );
        }
    }
}
