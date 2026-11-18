//! S0 probe of the QA accessibility layer (UI test redesign plan, L-A / O-A).
//!
//! GPUI controls have no UIKit accessibility nodes, so XCTest can only find
//! them by OCR. This layer exposes chosen GPUI controls as
//! `UIAccessibilityElement`s of a transparent UIView whose user interaction
//! is off: XCTest finds a control by identifier and its tap still lands on
//! the GPUI view underneath. One more element, `qa.state`, carries a small
//! JSON value that XCTest waits on instead of reading files the app writes.
//!
//! Only a Simulator build with `YEBISU_QA_AX` set builds the layer; any
//! other build draws nothing extra and never touches UIKit from here.
use gpui::{AnyElement, App, Bounds, Pixels, canvas, prelude::*};
use std::cell::RefCell;

/// The variable that turns the layer on in a Simulator build.
pub(crate) const ENV: &str = "YEBISU_QA_AX";
/// Whether this build may build the layer: only a Simulator build.
pub(crate) const BUILD: bool = cfg!(all(target_os = "ios", target_abi = "sim"));
/// Identifier of the tab row's ... (overflow menu) button.
pub(crate) const MORE: &str = "qa.shell.more";
/// Identifier of the state element whose value is `state_json`.
#[cfg_attr(
    not(any(all(target_os = "ios", target_abi = "sim"), test)),
    allow(dead_code, reason = "only the Simulator layer and the tests name it")
)]
pub(crate) const STATE: &str = "qa.state";

/// The rule: a Simulator build with `YEBISU_QA_AX` set.
pub(crate) fn allowed(simulator: bool, env_set: impl Fn(&str) -> bool) -> bool {
    simulator && env_set(ENV)
}

/// True only in a Simulator build with `YEBISU_QA_AX` set.
pub(crate) fn enabled() -> bool {
    allowed(BUILD, |name| std::env::var_os(name).is_some())
}

/// One exposed control: identifier, label and frame in window points
/// (x, y, width, height).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Control {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) frame: [f32; 4],
}

/// What `qa.state` reports. `seq` grows by one whenever another field
/// changes, so a waiter can tell a new value from a repeated one.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct State {
    pub(crate) seq: u64,
    pub(crate) presses: u64,
    pub(crate) overflow: bool,
    pub(crate) pasteboard: Option<String>,
}

/// The `qa.state` value: one line of JSON.
#[cfg_attr(
    not(any(all(target_os = "ios", target_abi = "sim"), test)),
    allow(dead_code, reason = "only the Simulator layer and the tests read it")
)]
pub(crate) fn state_json(state: &State) -> String {
    serde_json::json!({
        "seq": state.seq,
        "presses": state.presses,
        "overflow": state.overflow,
        "pasteboard": state.pasteboard,
    })
    .to_string()
}

/// The text the app puts on the pasteboard at press `presses`, so a probe
/// can tell whether the test runner reads what the app wrote.
pub(crate) fn pasteboard_nonce(pid: u32, presses: u64) -> String {
    format!("yebisu-qa-ax-{pid}-{presses}")
}

#[derive(Default)]
struct Probe {
    state: State,
    published: Option<(Vec<Control>, State)>,
}

thread_local! {
    static PROBE: RefCell<Probe> = RefCell::new(Probe::default());
}

/// Counts a press that GPUI delivered to a marked control and returns the
/// observation record for it; `None` (and nothing counted) when the layer
/// is off. In a Simulator build it also puts a fresh nonce on the
/// pasteboard and reports it in `qa.state`.
pub(crate) fn note_press(id: &'static str, overflow: bool) -> Option<serde_json::Value> {
    if !enabled() {
        return None;
    }
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        probe.state.presses += 1;
        let nonce = pasteboard_nonce(std::process::id(), probe.state.presses);
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        crate::qa_ax_native::set_pasteboard(&nonce);
        probe.state.pasteboard = Some(nonce.clone());
        Some(serde_json::json!({
            "kind": "qa-ax-press",
            "id": id,
            "presses": probe.state.presses,
            "overflow_before": overflow,
            "pasteboard": nonce,
        }))
    })
}

/// An absolutely placed canvas that records the parent's frame under `id`
/// at every paint and republishes the layer when anything changed. Add it
/// as a child of the control. `overflow` reads the menu state for
/// `qa.state`.
pub(crate) fn mark(
    id: &'static str,
    label: &'static str,
    overflow: impl Fn(&App) -> bool + 'static,
) -> Option<AnyElement> {
    if !enabled() {
        return None;
    }
    Some(
        canvas(
            |_, _, _| (),
            move |bounds, _, _, cx| {
                publish(
                    Control {
                        id,
                        label,
                        frame: frame(bounds),
                    },
                    overflow(cx),
                );
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

fn frame(bounds: Bounds<Pixels>) -> [f32; 4] {
    [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    ]
}

/// The next published value, or `None` when nothing changed since
/// `published`. A change in any field other than `seq` bumps `seq`.
pub(crate) fn next(
    published: Option<&(Vec<Control>, State)>,
    controls: Vec<Control>,
    mut state: State,
) -> Option<(Vec<Control>, State)> {
    if let Some((old_controls, old_state)) = published {
        state.seq = old_state.seq;
        if *old_controls == controls && *old_state == state {
            return None;
        }
        if *old_state != state {
            state.seq += 1;
        }
    } else {
        state.seq = 1;
    }
    Some((controls, state))
}

fn publish(control: Control, overflow: bool) {
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let mut state = probe.state.clone();
        state.overflow = overflow;
        let Some((controls, state)) = next(probe.published.as_ref(), vec![control], state) else {
            return;
        };
        let layout_changed = probe
            .published
            .as_ref()
            .is_none_or(|(old, _)| *old != controls);
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        crate::qa_ax_native::apply(&controls, &state_json(&state), layout_changed);
        #[cfg(not(all(target_os = "ios", target_abi = "sim")))]
        let _ = layout_changed;
        probe.state.seq = state.seq;
        probe.published = Some((controls, state));
    });
}

#[cfg(test)]
#[path = "qa_ax_tests.rs"]
mod tests;
