//! `qa.state` fields of the Herdr 2 core QA modes (UI test redesign S3:
//! P9 and SH1-154).
//!
//! Two kinds, both written only while the QA layer is on:
//!
//! * counters a QA mode bumps on GPUI's foreground thread when something
//!   finished (`qa_images`: the QA band prepared an image; `pasted_images`,
//!   `pasted_texts`, `paste_failures`: a paste ended). A test waits for one
//!   to grow instead of reading the result off the screen.
//! * the core worker's pane layout (`core`: panes, the active one, stacked
//!   or not, the terminal ids). The worker runs on its own thread, so it
//!   writes a shared slot that the root publisher reads on the next frame
//!   (`qa_ax::report` only accepts the publishing thread, review F4).
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, sync::Mutex};

thread_local! {
    /// Counts in this process; `qa.state` starts from zero on a relaunch.
    static COUNTS: RefCell<BTreeMap<&'static str, u64>> = const { RefCell::new(BTreeMap::new()) };
}

/// The last layout the core worker published (`layout`).
static LAYOUT: Mutex<Option<Value>> = Mutex::new(None);

/// Adds one to the counter `key` and reports it to `qa.state`. Call it on
/// GPUI's foreground thread. Does nothing unless the layer is on.
pub(crate) fn count(key: &'static str) {
    if !super::enabled() {
        return;
    }
    let value = COUNTS.with(|counts| {
        let mut counts = counts.borrow_mut();
        let value = counts.entry(key).or_default();
        *value += 1;
        *value
    });
    if !super::report(key, || value.into()) {
        eprintln!("qa_ax: {key} was not published (not the publishing thread)");
    }
}

/// The `core` field for a layout: `panes` screens, `active` the focused
/// one (0-based), `vertical` when stacked, `terminals` the ids in pane order.
pub(crate) fn layout_value(panes: usize, active: usize, vertical: bool, ids: &[&str]) -> Value {
    json!({"panes": panes, "active": active, "vertical": vertical, "terminals": ids})
}

/// Called by the core worker each time it publishes a frame. Any thread.
pub(crate) fn layout(
    panes: usize,
    active: usize,
    vertical: bool,
    ids: &[sheprd_backend::TerminalId],
) {
    if !super::enabled() {
        return;
    }
    let ids: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
    let value = layout_value(panes, active, vertical, &ids);
    *LAYOUT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value);
}

/// The last published layout, or null before the core worker published one.
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
pub(crate) fn layout_field() -> Value {
    LAYOUT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .unwrap_or(Value::Null)
}

#[cfg(test)]
#[path = "qa_ax_core_tests.rs"]
mod tests;
