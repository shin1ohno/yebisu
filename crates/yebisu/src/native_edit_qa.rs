//! Simulator-only selector-to-consumer checks; never included in device builds.
use crate::{app::Smoke, handoff::Selection};
use gpui::{AnyElement, ClipboardItem, Context, div, prelude::*, rgb};
use sheprd_client::protocol::ClientPaneInputEvent;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
static STEP: AtomicUsize = AtomicUsize::new(0);

impl Smoke {
    pub(crate) fn native_edit_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_NATIVE_QA")?;
        Some(div().id("native-next").p_2().bg(rgb(0x3b4252)).child("Native next")
            .on_click(cx.listener(|this, _, window, cx| {
                let step=STEP.fetch_add(1,Ordering::Relaxed);
                if step>0 {
                    let clipboard=cx.read_from_clipboard().and_then(|item|item.text());
                    this.observation.record(serde_json::json!({"kind":"native-result","case":step-1,"clipboard":clipboard,"preparing":this.handoff.preparing}));
                    this.native_marker(format!("native-end-{}",step-1));
                }
                if step>=24 { return; }
                this.terminal_focus.focus(window,cx);
                window.request_virtual_keyboard();
                this.terminal.input.options.command_control=step/6>=2;
                this.actions.terminal_priority=(step/6)%2==1;
                this.handoff.select_mode=false;
                this.handoff.selection=None;
                if let Ok(mut inbox)=this.terminal.link.inbox.lock() { inbox.keyboard_report_all=true; }
                this.refresh_terminal_input();
                this.sync_handoff();
                if step%6==1 || step%6==2 || (step%6==5 && !(this.terminal.input.options.command_control && this.actions.terminal_priority)) {
                    if let Some(surface)=&this.terminal.live.surface {
                        this.handoff.selection=Some(Selection {frame:Arc::new(surface.frame.clone()),anchor:(1,2),end:(6,2),dragging:false});
                    }
                }
                cx.write_to_clipboard(ClipboardItem::new_string(format!("native-text-{step}")));
                this.native_marker(format!("native-start-{step}"));
                let action=["paste:","copy:","cut:","selectAll:","physical-v","physical-c"][step%6];
                let scheduled=if step%6<4 { schedule_selector(action) } else {
                    let origin_none=herdr_uikit::native_edit_action().is_none();
                    let keystroke=gpui::Keystroke {key:if step%6==4 {"v"} else {"c"}.into(),key_char:None,modifiers:gpui::Modifiers {platform:true,..Default::default()}};
                    this.terminal_key_down(&gpui::KeyDownEvent {keystroke:keystroke.clone(),is_held:false,prefer_character_input:false},window,cx);
                    this.terminal_key_up(&gpui::KeyUpEvent {keystroke},window,cx);
                    origin_none
                };
                this.observation.record(serde_json::json!({"kind":"native-start","case":step,"action":action,"scheduled":scheduled,"command_control":this.terminal.input.options.command_control,"terminal_priority":this.actions.terminal_priority}));
                cx.notify();
            })).into_any_element())
    }
    fn native_marker(&self, text: String) {
        if let Some(handle) = &self.terminal.link.handle {
            let result = handle.send_input("boot-v1", "w1:p1", [ClientPaneInputEvent::Paste(text)]);
            if let Err(e) = result {
                tracing::error!(%e,"native QA marker failed");
            }
        }
    }
}

#[allow(
    unsafe_code,
    reason = "Simulator QA schedules a real SDK responder selector on UIKit's main run loop"
)]
fn schedule_selector(action: &str) -> bool {
    use objc2::{
        class, msg_send,
        rc::Retained,
        runtime::{AnyObject, Sel},
        sel,
    };
    use objc2_foundation::NSArray;
    // SAFETY: called only in the main-thread UI callback. UIKit owns each view;
    // retained arrays keep traversal nodes alive. No private SDK state is changed.
    unsafe fn visit(view: &AnyObject, selector: Sel) -> bool {
        unsafe {
            let matches = view
                .class()
                .name()
                .to_string_lossy()
                .contains("::GPUITextInputView");
            if matches {
                let _: () = msg_send![view,performSelector:selector withObject:std::ptr::null::<AnyObject>() afterDelay:0.1f64];
                return true;
            }
            let children: Retained<NSArray<AnyObject>> = msg_send![view, subviews];
            for child in children.iter() {
                if visit(&child, selector) {
                    return true;
                }
            }
            false
        }
    }
    let selector = match action {
        "paste:" => sel!(paste:),
        "copy:" => sel!(copy:),
        "cut:" => sel!(cut:),
        _ => sel!(selectAll:),
    };
    unsafe {
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        let windows: Retained<NSArray<AnyObject>> = msg_send![app, windows];
        windows.iter().any(|window| visit(&window, selector))
    }
}
