//! SDK11 positive control: each origin runs the original 24 receiving cases.
use crate::{app::Smoke, handoff::Selection};
use gpui::{AnyElement, ClipboardItem, Context, div, prelude::*, rgb};
use herdr_uikit::native_edit_observer as sdk;
use sheprd_client::protocol::ClientPaneInputEvent;
use std::sync::Arc;
impl Smoke {
    pub(crate) fn native_edit_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_NATIVE_QA")?;
        let label = if self.observer.failed {
            "Observer failed"
        } else if self.observer.phase > 24 {
            "Native matrix collected"
        } else {
            "Native next"
        };
        Some(div().id("native-next").p_2().bg(rgb(0x3b4252)).child(label)
            .on_click(cx.listener(|this,_,window,cx| {
                if this.observer.failed || this.observer.phase>24 {return;}
                if this.observer.phase>0 {
                    if !this.observer.complete() || this.handoff.workers!=0 || this.handoff.preparing {return;}
                    let i=this.observer.phase-1;
                    if !this.observer.end_sent {
                        let clipboard=cx.read_from_clipboard().and_then(|item|item.text());
                        let expected = if i%6==1 || i%6==2 || (i%6==5 && i/6!=3) {"日本語".to_owned()} else {format!("native-text-{i}")};
                        let clipboard_matches = clipboard.as_deref() == Some(expected.as_str());
                        if !this.observation.try_record(serde_json::json!({"kind":"native-result","case":i,"clipboard_matches":clipboard_matches,"preparing":this.handoff.preparing,"workers":this.handoff.workers}))
                            || !this.observed_native_marker("end",i) {this.observer.fail();return;}
                        this.observer.end_sent=true;return;
                    }
                    if !crate::mock_peer::observer_ack(this.observer.phase) {return;}
                    if !this.observation.try_record(serde_json::json!({"kind":"p11-peer-barrier","phase":this.observer.phase,"workers":this.handoff.workers,"preparing":this.handoff.preparing})) {this.observer.fail();return;}
                }
                this.observer.phase+=1;this.observer.end_sent=false;
                if this.observer.phase>24 {this.observer.finish(&this.observation);cx.notify();return;}
                let i=this.observer.phase-1;
                this.terminal_focus.focus(window,cx);window.request_virtual_keyboard();
                this.terminal.input.options.command_control=i/6>=2;
                this.actions.terminal_priority=(i/6)%2==1;
                this.handoff.select_mode=false;this.handoff.selection=None;
                if let Ok(mut inbox)=this.terminal.link.inbox.lock(){inbox.keyboard_report_all=true;}
                this.refresh_terminal_input();this.sync_handoff();
                if (i%6==1 || i%6==2 || (i%6==5 && i/6!=3)) && let Some(surface)=&this.terminal.live.surface {
                    this.handoff.selection=Some(Selection{frame:Arc::new(surface.frame.clone()),anchor:(1,2),end:(6,2),dragging:false});
                }
                cx.write_to_clipboard(ClipboardItem::new_string(format!("native-text-{i}")));
                let action=["paste:","copy:","cut:","selectAll:","physical-v","physical-c"][(i%6)as usize];
                if !this.observed_native_marker("start",i) || !this.observation.try_record(serde_json::json!({"kind":"native-start","case":i,"action":action,"command_control":this.terminal.input.options.command_control,"terminal_priority":this.actions.terminal_priority})) {this.observer.fail();return;}
                if i%6<4 {
                    let action=[sdk::Action::Paste,sdk::Action::Copy,sdk::Action::Cut,sdk::Action::SelectAll][(i%6)as usize];
                    let origin=if std::env::var_os("YEBISU_P11_DRAWING").is_some(){sdk::ResponderOrigin::Drawing}else{sdk::ResponderOrigin::Text};
                    let policy=this.native_edit_availability(window);
                    this.observer.start(window.window_handle(),&this.observation,action,origin,policy,policy);
                } else {
                    // Existing physical-key adapter control; not hardware proof.
                    let origin_none=herdr_uikit::native_edit_action().is_none() && sdk::current_native_observation_key().is_none();
                    let key=gpui::Keystroke{key:if i%6==4{"v"}else{"c"}.into(),key_char:None,modifiers:gpui::Modifiers{platform:true,..Default::default()}};
                    this.terminal_key_down(&gpui::KeyDownEvent{keystroke:key.clone(),is_held:false,prefer_character_input:false},window,cx);
                    this.terminal_key_up(&gpui::KeyUpEvent{keystroke:key},window,cx);
                    if !this.observation.try_record(serde_json::json!({"kind":"p11-physical-control","case":i,"origin_none":origin_none})) || !origin_none {this.observer.fail();}
                }
                cx.notify();
            })).into_any_element())
    }
    fn observed_native_marker(&self, edge: &str, i: u32) -> bool {
        self.terminal.link.handle.as_ref().is_some_and(|h| {
            h.send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste(format!("native-{edge}-{i}"))],
            )
            .is_ok()
        })
    }
}
