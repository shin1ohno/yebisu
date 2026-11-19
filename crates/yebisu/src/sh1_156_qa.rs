//! Simulator-only SH1-156 fixture for scripts/ios/sh1-156-check.py: leaves a
//! discarded composition, as the owner's device had before the failing click.
//! The composition goes through the adapter UIKit's setMarkedText calls and the
//! production discard. It sends no input: the XCTest clicks and types through
//! UIKit, and the check reads the private peer's receipts.
use crate::app::Smoke;
use gpui::{AnyElement, Context, EntityInputHandler, div, prelude::*, rgb};

/// The control's text, which is also its accessibility label.
const DISCARD: &str = "Discard composition";

impl Smoke {
    pub(crate) fn sh1_156_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_SH1_156_QA")?;
        Some(
            div()
                .id("sh1-156-discard")
                .p_2()
                .bg(rgb(0x3b4252))
                .child(DISCARD)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.terminal_focus.focus(window, cx);
                    this.refresh_terminal_input();
                    // The user was typing: the composition starts as fresh input.
                    this.terminal.input.fresh_gesture();
                    this.replace_and_mark_text_in_range(None, "てすと", Some(0..3), window, cx);
                    let marked = this.terminal.input.composition.text.clone();
                    this.terminal.input.discard();
                    this.observation.record(serde_json::json!({
                        "kind": "sh1-156-discarded",
                        "marked": marked,
                        "fenced": this.terminal.input.requires_gesture(),
                        "drag_select": this.handoff.drag_select,
                        "select_mode": this.handoff.select_mode,
                    }));
                    window.notify_text_input_changed(&this.terminal_focus);
                    crate::qa_ax::count("sh156_discarded");
                    cx.notify();
                }))
                .children(crate::qa_ax::mark(
                    crate::qa_ax::ids::SH1_156_DISCARD,
                    DISCARD,
                ))
                .into_any_element(),
        )
    }
}
