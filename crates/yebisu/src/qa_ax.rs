//! The QA accessibility layer (UI test redesign plan 3.4, L-A / O-A, S1).
//!
//! GPUI controls have no UIKit accessibility nodes, so XCTest could find them
//! only by reading the screen. This layer exposes marked GPUI controls as
//! `UIAccessibilityElement`s of one transparent UIView whose user interaction
//! is off: XCTest finds a control by identifier, and its tap still lands on
//! the GPUI view underneath as a real touch. One more element, `qa.state`,
//! exists from the first frame (it is published from the app root, not from
//! a marked control) and carries one line of JSON that XCTest reads directly.
//!
//! The S0 probe amended the plan in two ways that this layer keeps:
//! `isHittable` does not see GPUI occlusion, so a press is confirmed only by
//! `qa.state` (each mark counts the presses GPUI hit-tests to it in
//! `presses`); and `qa.state` is there from launch, on every screen.
//!
//! Only a Simulator build with `YEBISU_QA_AX` set builds the layer. In any
//! other build `mark`, `root` and `report` return at once, the layer module
//! is not compiled and UIKit is never touched from here.
use gpui::AnyElement;

#[path = "qa_ax_ids.rs"]
pub(crate) mod ids;
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
#[path = "qa_ax_layer.rs"]
pub(crate) mod layer;
#[cfg(all(target_os = "ios", target_abi = "sim"))]
#[path = "qa_ax_native.rs"]
mod native;
#[path = "qa_ax_root.rs"]
mod root;

/// The variable that turns the layer on in a Simulator build.
pub(crate) const ENV: &str = "YEBISU_QA_AX";
/// Whether this build may build the layer: only a Simulator build.
pub(crate) const BUILD: bool = cfg!(all(target_os = "ios", target_abi = "sim"));

/// The rule: a Simulator build with `YEBISU_QA_AX` set.
pub(crate) fn allowed(simulator: bool, env_set: impl Fn(&str) -> bool) -> bool {
    simulator && env_set(ENV)
}

/// True only in a Simulator build with `YEBISU_QA_AX` set.
pub(crate) fn enabled() -> bool {
    allowed(BUILD, |name| std::env::var_os(name).is_some())
}

/// The mark for a control: add the returned element as a child of the
/// control (it is absolutely placed over the control's bounds). `id` is a
/// constant from `ids`; `label` is the same constant the control paints, so
/// the accessibility label and the drawn text cannot drift apart. `None`
/// unless the layer is on.
pub(crate) fn mark(id: &'static str, label: &'static str) -> Option<AnyElement> {
    if !enabled() {
        return None;
    }
    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    {
        Some(layer::marker(id, label))
    }
    #[cfg(not(any(all(target_os = "ios", target_abi = "sim"), test)))]
    {
        let _ = (id, label);
        None
    }
}

/// Sets one field of `qa.state` (for a QA mode's own state, such as a key
/// being ready); it is published with the next frame. Returns false, and
/// changes nothing, when the layer is off or `key` is one of the layer's own
/// fields (`layer::RESERVED`).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "S1 lays the API down; S2 (p2 key and pasteboard state) is its first caller"
    )
)]
pub(crate) fn report(key: &'static str, value: impl FnOnce() -> serde_json::Value) -> bool {
    if !enabled() {
        return false;
    }
    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    {
        layer::report(key, value())
    }
    #[cfg(not(any(all(target_os = "ios", target_abi = "sim"), test)))]
    {
        let _ = (key, value);
        false
    }
}

#[cfg(test)]
#[path = "qa_ax_tests.rs"]
mod tests;
