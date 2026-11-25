//! Simulator QA (#139): `qa.state.input_mode`, the SDK's selection display
//! next to the terminal's own cursor and composition rectangle, published with
//! each drawn frame. `scripts/ios/input-mode-check.py` reads it. The SDK's
//! state is sampled without its input handler, so this is safe while drawing.

use super::TerminalView;
use std::{cell::RefCell, collections::BTreeMap};

thread_local! {
    /// The SDK state last seen per window, so a change asks for one frame.
    static SEEN: RefCell<BTreeMap<String, String>> = const { RefCell::new(BTreeMap::new()) };
    /// Per window, the indicator as the poll saw it (`shown` in qa.state).
    static SHOWN: RefCell<BTreeMap<String, Shown>> = const { RefCell::new(BTreeMap::new()) };
}

/// How often the indicator appeared, its last visible frame and the caret
/// then. XCUITest can wait 60 s for the app to idle after a key, past the
/// indicator's display, so the check reads this instead of hoping to catch
/// the indicator on screen.
#[derive(Default)]
struct Shown {
    count: u64,
    visible: bool,
    frame: Option<[f64; 4]>,
    caret: Option<[f64; 4]>,
}

/// From the terminal poll (every 33 ms): UIKit shows and hides the
/// indicator without a GPUI frame, and `qa.state` is published only with a
/// frame, so a changed SDK state asks for one.
pub(super) fn poll(cx: &mut gpui::App) {
    if !crate::qa_ax::enabled() {
        return;
    }
    for handle in cx.windows() {
        let sdk = herdr_uikit::input_mode_indicator_state(handle);
        let window = format!("{:?}", handle.window_id());
        let frame = sdk.as_ref().and_then(|s| s.accessory_frame);
        SHOWN.with_borrow_mut(|shown| {
            let shown = shown.entry(window.clone()).or_default();
            if frame.is_some() {
                shown.count += u64::from(!shown.visible);
                shown.frame = frame;
                shown.caret = sdk.as_ref().and_then(|s| s.caret_rect);
            }
            shown.visible = frame.is_some();
        });
        let state = format!("{sdk:?}");
        let changed =
            SEEN.with_borrow_mut(|seen| seen.insert(window, state.clone()) != Some(state));
        if changed {
            crate::qa_ax::request_frame();
        }
    }
}

/// `[x, y, width, height]` in window points.
fn rect(bounds: gpui::Bounds<gpui::Pixels>) -> [f32; 4] {
    [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    ]
}

pub(super) fn report(terminal: &TerminalView, face: &gpui::Font, window: &gpui::Window) {
    crate::qa_ax::report("input_mode", || {
        let surface = terminal.live.surface.as_deref();
        let cursor = terminal.geometry.cursor(surface);
        let composition = &terminal.input.composition;
        // The range `bounds_for_range` places for the IME (`input.rs`).
        let range = if terminal.input.options.caret_anchor {
            composition.selected.start..composition.selected.start
        } else {
            composition.selected.clone()
        };
        let composition_bounds = terminal.painter.composition_bounds(
            &composition.text,
            range,
            cursor,
            terminal.geometry.area(surface),
            face,
            window,
        );
        let sdk = herdr_uikit::input_mode_indicator_state(window.window_handle());
        let shown = SHOWN.with_borrow(|shown| {
            let shown = shown.get(&format!("{:?}", window.window_handle().window_id()));
            serde_json::json!({
                "count": shown.map_or(0, |s| s.count),
                "frame": shown.and_then(|s| s.frame),
                "caret": shown.and_then(|s| s.caret),
            })
        });
        serde_json::json!({
            "text_first_responder": sdk.as_ref().map(|s| s.text_first_responder),
            "activated": sdk.as_ref().map(|s| s.activated),
            "interactions": sdk.as_ref().map(|s| s.interactions_on_drawing_view),
            "text_input_is_current": sdk.as_ref().map(|s| s.text_input_is_current),
            "cursor_class": sdk.as_ref().map(|s| s.cursor_class.clone()),
            "accessory_frame": sdk.as_ref().and_then(|s| s.accessory_frame),
            "caret_rect": sdk.as_ref().and_then(|s| s.caret_rect),
            "needs_update_calls": sdk.as_ref().map(|s| s.needs_update_calls),
            "cursor": rect(cursor),
            "composition_bounds": rect(composition_bounds),
            "marked": composition.text,
            "invalidations": terminal.input_indicator.invalidations(),
            "shown": shown,
        })
    });
}
