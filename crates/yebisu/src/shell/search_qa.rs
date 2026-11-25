//! Simulator QA for sidebar search (test-support Simulator builds only).
//!
//! Each frame the magnifier paints, `qa.state.search` carries yebisu's side
//! of the session (phase, filter, presentation, the revision at open, the
//! last event and the numbered recent ones) and the SDK's (`herdr_uikit::native_search_state`: which view
//! is first responder, the field's text, the presentation style). The same
//! object goes to app.jsonl as a `search-state` record whenever it changes,
//! so `scripts/ios/search-check.py` can judge a step after the run even when
//! XCUITest cannot read `qa.state` while the panel is up. While a session
//! exists the layer asks for frames, so the state keeps being published.
use super::search::{Phase, Presentation, SearchEvent};
use crate::{app::Smoke, observation::Observation};
use gpui::AnyWindowHandle;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::VecDeque};

/// `1` makes the panel a sheet and `0` a popover, whatever the width: the
/// Simulator has no window below 352 pt (issue #138, T-S9).
pub(crate) const COMPACT_ENV: &str = "YEBISU_QA_SEARCH_COMPACT";

/// How many recent events a record carries. A `Dismissed` and the
/// `InputRestored` after it can land in one frame, so `last_event` alone
/// can skip the dismissal.
const RECENT_EVENTS: usize = 16;

thread_local! {
    static LAST_EVENT: RefCell<String> = const { RefCell::new(String::new()) };
    /// The number of events so far and the last `RECENT_EVENTS` of them.
    static EVENTS: RefCell<(u64, VecDeque<String>)> = const { RefCell::new((0, VecDeque::new())) };
    static LAST_RECORD: RefCell<Option<Value>> = const { RefCell::new(None) };
}

pub(crate) fn forced_compact() -> Option<bool> {
    match std::env::var(COMPACT_ENV).ok().as_deref() {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => None,
    }
}

pub(crate) fn event(_app: &Smoke, event: &SearchEvent) {
    let text = format!("{event:?}");
    EVENTS.with_borrow_mut(|(count, recent)| {
        *count += 1;
        recent.push_back(text.clone());
        if recent.len() > RECENT_EVENTS {
            recent.pop_front();
        }
    });
    LAST_EVENT.with_borrow_mut(|last| *last = text);
}

/// `[[number, event], ...]` for the recent events, numbered from 1.
fn recent_events() -> Value {
    EVENTS.with_borrow(|(count, recent)| {
        let first = count.saturating_sub(recent.len() as u64) + 1;
        recent
            .iter()
            .zip(first..)
            .map(|(event, number)| json!([number, event]))
            .collect()
    })
}

/// yebisu's side, taken while rendering the magnifier.
pub(crate) fn snapshot(app: &Smoke) -> Value {
    let session = app.shell.search.session.as_ref();
    json!({
        "open": session.is_some(),
        "phase": session.map(|s| match s.phase {
            Phase::Presenting => "Presenting",
            Phase::Open => "Open",
            Phase::Closing => "Closing",
            Phase::Restoring => "Restoring",
        }),
        "presentation": session.map(|s| match s.presentation {
            Presentation::Popover => "Popover",
            Presentation::Sheet => "Sheet",
        }),
        "revision_at_open": session.map(|s| s.revision_at_open),
        "pending": session.and_then(|s| s.pending).map(|(revision, id)| json!([revision, id])),
        "filter": app.sidebar_filter(),
        "last_event": LAST_EVENT.with_borrow(Clone::clone),
        "events": recent_events(),
        "status": app.terminal.live.status,
        "sidebar_mode": format!("{:?}", app.sidebar_user_mode()),
        "find_open": app.find.is_some(),
    })
}

/// Publishes `mine` with the SDK's state of `window`; called from paint.
pub(crate) fn publish(window: AnyWindowHandle, mut mine: Value, observation: &Observation) {
    if let Some(sdk) = herdr_uikit::native_search_state(window) {
        mine["sdk"] = json!({
            "phase": sdk.phase,
            "session": sdk.session,
            "field_first_responder": sdk.field_first_responder,
            "gpui_text_first_responder": sdk.gpui_text_first_responder,
            "gpui_drawing_first_responder": sdk.gpui_drawing_first_responder,
            "field_text": sdk.field_text,
            "field_marked": sdk.field_marked,
            "presentation_style": sdk.presentation_style,
            "late_drops": sdk.late_drops,
            "dismissals_begun": sdk.dismissals_begun,
        });
    }
    let open = mine["open"].as_bool().unwrap_or(false);
    crate::qa_ax::report("search", || mine.clone());
    let changed = LAST_RECORD.with_borrow_mut(|last| {
        let changed = last.as_ref() != Some(&mine);
        if changed {
            *last = Some(mine.clone());
        }
        changed
    });
    if changed {
        let mut record = json!({"kind": "search-state"});
        if let (Some(record), Some(fields)) = (record.as_object_mut(), mine.as_object()) {
            record.extend(fields.clone());
        }
        observation.record_without_waiting(record);
    }
    if open {
        crate::qa_ax::request_frame();
    }
}
