//! Simulator-only SH1-153 checks through UIKit validation and the active responder.
use crate::{app::Smoke, handoff::Selection};
use gpui::{AnyElement, ClipboardItem, Context, div, prelude::*, rgb};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
static CASE: AtomicUsize = AtomicUsize::new(0);

impl Smoke {
    pub(crate) fn edit_menu_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_EDIT_MENU_QA")?;
        Some(
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .id("edit-prepare")
                        .p_2()
                        .bg(rgb(0x3b4252))
                        .child("Menu prepare")
                        .on_click(cx.listener(|this, _, window, cx| {
                            // Keep observation enabled, but do not let the smoke
                            // timer repair stale native availability every second.
                            this.smoke_mode = false;
                            let step = CASE.fetch_add(1, Ordering::Relaxed);
                            if step > 0 {
                                this.observation.record(
                                    serde_json::json!({"kind":"edit-result", "case":step-1,
                            "clipboard":cx.read_from_clipboard().and_then(|item| item.text())}),
                                );
                                this.native_marker(format!("edit-end-{}", step - 1));
                            }
                            if step >= 10 {
                                return;
                            }
                            this.actions.palette = false;
                            this.terminal_focus.focus(window, cx);
                            if step < 5 {
                                window.request_virtual_keyboard();
                            } else {
                                window.dismiss_virtual_keyboard();
                            }
                            this.refresh_terminal_input();
                            this.sync_handoff();
                            this.handoff.selection = None;
                            if [1, 4].contains(&(step % 5))
                                && let Some(surface) = &this.terminal.live.surface
                            {
                                this.handoff.selection = Some(Selection {
                                    frame: Arc::new(surface.frame.clone()),
                                    anchor: (1, 2),
                                    end: (6, 2),
                                    dragging: false,
                                });
                            }
                            if step % 5 == 3 {
                                window.blur(cx);
                            }
                            cx.write_to_clipboard(ClipboardItem::new_string(format!(
                                "edit-text-{step}"
                            )));
                            this.native_marker(format!("edit-start-{step}"));
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("edit-execute")
                        .p_2()
                        .bg(rgb(0x3b4252))
                        .child("Menu execute")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let step = CASE.load(Ordering::Relaxed).saturating_sub(1);
                            let copy = [1, 2, 4].contains(&(step % 5));
                            let result = probe_and_schedule(copy);
                            this.observation
                                .record(serde_json::json!({"kind":"edit-validation", "case":step,
                        "probe":result}));
                            // The action was validated with a selection but runs after it disappears.
                            if step % 5 == 4 {
                                this.handoff.selection = None;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}

#[allow(
    unsafe_code,
    reason = "Simulator QA invokes public UIKit responder methods on the main thread"
)]
fn probe_and_schedule(copy: bool) -> Option<serde_json::Value> {
    use objc2::{Message, class, msg_send, rc::Retained, runtime::AnyObject, sel};
    use objc2_foundation::{NSArray, NSString};
    use objc2_ui_kit::{UICommand, UIMenuElementAttributes};
    // SAFETY: retained UIKit views are traversed on the main thread only.
    unsafe fn first(view: &AnyObject) -> Option<Retained<AnyObject>> {
        unsafe {
            let active: bool = msg_send![view, isFirstResponder];
            if active {
                return Some(view.retain());
            }
            let children: Retained<NSArray<AnyObject>> = msg_send![view, subviews];
            children.iter().find_map(|child| first(&child))
        }
    }
    // SAFETY: public UIKit methods with their declared signatures, called from a UI callback.
    unsafe {
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        let windows: Retained<NSArray<AnyObject>> = msg_send![app, windows];
        let responder = windows.iter().find_map(|window| first(&window))?;
        let selector = if copy { sel!(copy:) } else { sel!(paste:) };
        let enabled: bool = msg_send![&*responder, canPerformAction:selector, withSender:std::ptr::null::<AnyObject>()];
        let mtm = objc2::MainThreadMarker::new()?;
        let command = UICommand::commandWithTitle_image_action_propertyList(
            &NSString::from_str(if copy { "Copy" } else { "Paste" }),
            None,
            selector,
            None,
            mtm,
        );
        // Start disabled so validateCommand must actively enable a valid command.
        command.setAttributes(UIMenuElementAttributes::Disabled);
        let _: () = msg_send![&*responder, validateCommand:&*command];
        let validated = !command
            .attributes()
            .contains(UIMenuElementAttributes::Disabled);
        if enabled && validated {
            let _: () = msg_send![&*responder, performSelector:selector,
                withObject:std::ptr::null::<AnyObject>(), afterDelay:0.1f64];
        }
        Some(serde_json::json!({"enabled":enabled,"validated":validated,
            "responder":responder.class().name().to_string_lossy()}))
    }
}
