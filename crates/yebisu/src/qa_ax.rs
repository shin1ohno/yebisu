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

#[path = "qa_ax_core.rs"]
pub(crate) mod core;
#[path = "qa_ax_ids.rs"]
pub(crate) mod ids;
#[path = "qa_ax_keys.rs"]
mod keys;
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
#[path = "qa_ax_layer.rs"]
pub(crate) mod layer;
#[cfg(all(target_os = "ios", target_abi = "sim"))]
#[path = "qa_ax_native.rs"]
mod native;
#[path = "qa_ax_peer.rs"]
pub(crate) mod peer;
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

/// The launch variable that sets how a selection starts (UI test redesign
/// S3): P9 launches with `double-tap` instead of opening Settings › Input ›
/// Start a selection before its test, and checks `drag_select` in `qa.state`.
pub(crate) const START_SELECTION_ENV: &str = "YEBISU_QA_START_SELECTION";

/// The rule: a selection starts by dragging (the product default) unless a
/// Simulator build was launched with `YEBISU_QA_START_SELECTION=double-tap`.
pub(crate) fn start_drag_select(simulator: bool, value: Option<&str>) -> bool {
    !(simulator && value == Some("double-tap"))
}

/// The start mode at launch (`start_drag_select` for this build).
pub(crate) fn launch_drag_select() -> bool {
    let value = std::env::var(START_SELECTION_ENV).ok();
    start_drag_select(BUILD, value.as_deref())
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
/// being ready); it is published with the next frame. Call it on GPUI's
/// foreground thread (in a render or an event listener): the fields live in
/// that thread's registry, which the root publisher reads. Returns false, and
/// changes nothing, when the layer is off, `key` is one of the layer's own
/// fields (`layer::RESERVED`), or this thread never built the root publisher
/// (a background task, or before the first frame).
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

/// A frame is wanted although no view may have changed: a counter or the
/// peer moved (`request_frame`), and `qa.state` is published only by a frame.
static FRAME_WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Asks for a frame so the next `qa.state` carries what just changed. Any
/// thread; `refresh_if_wanted` turns it into a refresh on the next poll.
pub(crate) fn request_frame() {
    FRAME_WANTED.store(true, std::sync::atomic::Ordering::Release);
}

/// Called from the terminal poll (every 33 ms on GPUI's foreground thread):
/// refreshes every window once a frame was requested. Nothing unless the
/// layer is on.
pub(crate) fn refresh_if_wanted(cx: &mut gpui::App) {
    if enabled() && FRAME_WANTED.swap(false, std::sync::atomic::Ordering::AcqRel) {
        cx.refresh_windows();
    }
}

/// Adds one to the `qa.state` counter `key` (UI test redesign S3/S4: a
/// test waits for the count instead of reading the app's receipt files) and
/// requests the frame that publishes it. Call it on GPUI's foreground thread.
/// Does nothing unless the layer is on.
pub(crate) fn count(key: &'static str) {
    if !enabled() {
        return;
    }
    core::count(key);
    request_frame();
}

/// The `qa.state` counter for a lifecycle phase (`lifecycle_active`, ...).
pub(crate) fn lifecycle_key(phase: gpui::AppLifecyclePhase) -> &'static str {
    use gpui::AppLifecyclePhase as Phase;
    match phase {
        Phase::Active => "lifecycle_active",
        Phase::Inactive => "lifecycle_inactive",
        Phase::Background => "lifecycle_background",
        Phase::Foreground => "lifecycle_foreground",
        Phase::Disconnected => "lifecycle_disconnected",
    }
}

#[cfg(test)]
#[path = "qa_ax_tests.rs"]
mod tests;
