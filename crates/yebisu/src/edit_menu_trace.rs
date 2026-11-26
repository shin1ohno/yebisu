//! Simulator QA: what became of each edit menu the app presented (issue #163).
//!
//! L61 (`EditMenuTargetsQA`) failed on CI with the app's own receipts saying
//! the secondary click arrived and the menu was presented, while XCUITest
//! found no Paste item. herdr-uikit presents on the next main-queue turn and
//! UIKit can still refuse or close the menu, so after each presentation the
//! app records, at fixed delays, herdr-uikit's status of the menu's token and,
//! per UIKit window, whether it is key, how many edit menu views it holds and
//! which kind of view is first responder. Only static codes, enum names and
//! counts are recorded: no labels, no terminal text.

/// Milliseconds after a presentation at which the trace is recorded.
pub(crate) const DELAYS_MS: [u64; 4] = [30, 300, 1000, 3000];

/// The kind of a UIKit window, from its class name.
pub(crate) fn window_kind(class: &str) -> &'static str {
    match class {
        "UIWindow" => "app",
        "UITextEffectsWindow" => "text_effects",
        _ => "other",
    }
}

/// Whether a view of this class is part of UIKit's edit menu.
pub(crate) fn is_edit_menu_view(class: &str) -> bool {
    class.starts_with("_UIEditMenu")
}

/// The kind of a window's first responder, from its class name: GPUI's text
/// input view, its drawing view, something else, or none.
pub(crate) fn responder_kind(class: Option<&str>) -> &'static str {
    match class {
        None => "none",
        Some(class) if class.contains("GPUITextInputView") => "text_input",
        Some(class) if class.contains("GPUIView") => "drawing",
        Some(_) => "other",
    }
}

/// One UIKit window as the trace records it.
pub(crate) fn window_record(
    class: &str,
    key: bool,
    menu_views: usize,
    responder: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "window": window_kind(class),
        "key": key,
        "menu_views": menu_views,
        "responder": responder_kind(responder),
    })
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
impl crate::app::Smoke {
    /// Records the trace of the menu presented with `token` at each of
    /// `DELAYS_MS`, while `qa.state` is on (`YEBISU_QA_AX`).
    pub(crate) fn trace_edit_menu(
        &self,
        token: crate::edit_menu::MenuToken,
        handle: gpui::AnyWindowHandle,
        cx: &mut gpui::Context<Self>,
    ) {
        if std::env::var_os("YEBISU_QA_AX").is_none() {
            return;
        }
        #[cfg(not(test))]
        let crate::edit_menu::MenuToken::Native(token) = token;
        #[cfg(test)]
        let token = match token {
            crate::edit_menu::MenuToken::Native(token) => token,
            crate::edit_menu::MenuToken::Recorded(_) => return,
        };
        for delay in DELAYS_MS {
            cx.spawn(async move |weak, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(delay))
                    .await;
                // On the main thread, outside a GPUI update, as sh1_152_capture reads UIKit.
                let status = format!("{:?}", herdr_uikit::native_edit_menu_status(handle, token));
                let windows = crate::edit_menu_qa_native::edit_menu_windows();
                let _ = weak.update(cx, |this, _| {
                    this.observation.record(serde_json::json!({
                        "kind": "edit-menu-trace", "token": token.value(), "after_ms": delay,
                        "status": status, "windows": windows,
                    }));
                });
            })
            .detach();
        }
    }
}

#[cfg(test)]
#[path = "edit_menu_trace_tests.rs"]
mod tests;
