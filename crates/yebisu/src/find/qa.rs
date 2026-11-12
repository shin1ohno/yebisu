//! Simulator-only production callback exercise. Physical IME remains owner-only.
use crate::app::Smoke;
use gpui::{prelude::*, *};
use std::sync::atomic::{AtomicUsize, Ordering};
static STEP: AtomicUsize = AtomicUsize::new(0);
impl Smoke {
    pub(crate) fn p13_qa(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_P13_QA")?;
        Some(div().id("p13-next").px_4().py_2().child("P13 Next")
            .on_click(cx.listener(|this,_,window,cx| {
                let step=STEP.fetch_add(1,Ordering::Relaxed);
                if step==0 { this.open_find(window,cx); }
                else if step==13 {
                    let mut repeat=qa_key("escape");repeat.is_held=true;
                    this.terminal_key_down(&repeat,window,cx);
                    this.terminal_key_up(&KeyUpEvent{keystroke:qa_key("escape").keystroke},window,cx);
                    let focused=this.terminal_focus.is_focused(window);
                    this.terminal_key_down(&qa_key("f"),window,cx);
                    this.replace_text_in_range(None,"P13-fresh",window,cx);
                    this.observation.record(serde_json::json!({"kind":"p13-close","focused":focused,"active":this.find.is_some()}));
                } else if let Some(bar)=&this.find {
                    let field=bar.field.clone();
                    let focus=field.read(cx).focus.clone(); focus.focus(window,cx);
                    field.update(cx,|field,cx| {
                        match step {
                            1|2 => {
                                field.input.selected=0..field.input.text.encode_utf16().count();
                                field.replace_text_in_range(None,if step==1 {"needle"}else{"日本語"},window,cx);
                            }
                            3 => {
                                field.replace_and_mark_text_in_range(None,"変😀中",Some(1..3),window,cx);
                                let span=field.input.marked.clone();
                                if let Some(span)=span {
                                    field.replace_text_in_range(Some(span.start+1..span.start+3),"",window,cx);
                                    this.observation.record(serde_json::json!({"kind":"p13-partial-mark","retained":field.input.marked.is_some()}));
                                }
                                field.key(&qa_key("escape"),window,cx);
                            }
                            4 => field.key(&qa_key("enter"),window,cx),
                            5 => field.key(&qa_key("shift-enter"),window,cx),
                            6 => {
                                field.input.selected=0..field.input.text.encode_utf16().count();
                                cx.write_to_clipboard(ClipboardItem::new_string("needle"));
                            }
                            7 | 9 | 10 => {
                                field.input.set_text("pre::post".into());
                                field.replace_and_mark_text_in_range(Some(4..4),"変😀中",Some(1..3),window,cx);
                                if step==9 {
                                    field.key(&qa_key("cmd-x"),window,cx);
                                } else if step==10 {
                                    field.replace_text_in_range(None,"確定",window,cx);
                                    this.observation.record(serde_json::json!({"kind":"p13-ime-none","text":field.input.text,"marked":field.input.marked}));
                                }
                            }
                            11 => {
                                field.input.selected=0..field.input.text.encode_utf16().count();
                                field.replace_text_in_range(None,"beyond",window,cx);
                            }
                            12 => field.key(&qa_key("escape"),window,cx),
                            _=>{}
                        }
                        if step==8 || step==9 {
                            this.observation.record(serde_json::json!({"kind":"p13-cut","action":if step==8 {"nativeCut"}else{"commandX"},"text":field.input.text,"marked":field.input.marked,"selected":[field.input.selected.start,field.input.selected.end],"clipboard":cx.read_from_clipboard().and_then(|item|item.text())}));
                            field.key(&qa_key("escape"),window,cx);
                            this.observation.record(serde_json::json!({"kind":"p13-cut-cancel","step":step,"text":field.input.text,"marked":field.input.marked}));
                        }
                        if (8..=10).contains(&step) {
                            field.input.selected=0..field.input.text.encode_utf16().count();
                            field.replace_text_in_range(None,"needle",window,cx);
                        }
                    });
                    if step==6 || step==7 {
                        let scheduled=schedule_edit(step==7);
                        this.observation.record(serde_json::json!({"kind":"p13-native-selector","action":if step==7 {"cut"}else{"paste"},"scheduled":scheduled}));
                    }
                }
                this.observation.record(serde_json::json!({"kind":"p13-step","step":step,"active":this.find.is_some(),"marked":this.find.as_ref().and_then(|b|b.field.read(cx).input.marked.clone())}));
                cx.notify();
            })).into_any_element())
    }
}
fn qa_key(name: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap_or_default(),
        is_held: false,
        prefer_character_input: false,
    }
}
#[allow(
    unsafe_code,
    reason = "Simulator QA invokes UIKit's real edit selectors on its main run loop"
)]
fn schedule_edit(cut: bool) -> bool {
    use objc2::{class, msg_send, rc::Retained, runtime::AnyObject, sel};
    use objc2_foundation::NSArray;
    // SAFETY: UI-thread only. Retained UIKit arrays own each traversal node.
    unsafe fn visit(view: &AnyObject, cut: bool) -> bool {
        unsafe {
            if view
                .class()
                .name()
                .to_string_lossy()
                .contains("::GPUITextInputView")
            {
                let selector = if cut { sel!(cut:) } else { sel!(paste:) };
                let _: () = msg_send![view,performSelector:selector,withObject:std::ptr::null::<AnyObject>(),afterDelay:0.1f64];
                return true;
            }
            let children: Retained<NSArray<AnyObject>> = msg_send![view, subviews];
            children.iter().any(|child| visit(&child, cut))
        }
    }
    unsafe {
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        let windows: Retained<NSArray<AnyObject>> = msg_send![app, windows];
        windows.iter().any(|window| visit(&window, cut))
    }
}
