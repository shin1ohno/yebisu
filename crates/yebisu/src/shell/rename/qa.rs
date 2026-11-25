//! Simulator QA (issue #143): the rename's state in `qa.state.rename`,
//! yebisu's side and the SDK's field (`native_inline_field_state`), so a
//! SmokeQA test confirms by state rather than by pixels.
use super::{Phase, RenameAnchor, RenameObject};
use crate::app::Smoke;
use serde_json::{Value, json};

#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
thread_local! {
    /// The last state recorded, so a record is written only when it changes.
    static LAST_RECORD: std::cell::RefCell<Option<Value>> = const { std::cell::RefCell::new(None) };
}

fn object(object: &RenameObject) -> Value {
    match object {
        RenameObject::Pane(id) => json!(["pane", id]),
        RenameObject::Space(id) => json!(["space", id]),
        RenameObject::Tab(id) => json!(["tab", id]),
    }
}

fn rect(bounds: gpui::Bounds<gpui::Pixels>) -> Value {
    json!([
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height)
    ])
}

/// yebisu's side of the rename.
pub(crate) fn snapshot(app: &Smoke) -> Value {
    let rename = &app.shell.rename;
    let session = rename.session.as_ref();
    let slot = session.and_then(|s| rename.slots.last(s.anchor, &s.target.object));
    json!({
        "open": session.is_some(),
        "phase": session.map(|s| match s.phase {
            Phase::Measuring => "Measuring",
            Phase::Presenting => "Presenting",
            Phase::Open => "Open",
            Phase::Restoring => "Restoring",
        }),
        "anchor": session.map(|s| match s.anchor {
            RenameAnchor::AgentRow => "AgentRow",
            RenameAnchor::PaneChip => "PaneChip",
            RenameAnchor::RailHeader => "RailHeader",
            RenameAnchor::SpaceRow => "SpaceRow",
            RenameAnchor::TabLabel => "TabLabel",
        }),
        "object": session.map(|s| object(&s.target.object)),
        "slot": slot.map(|p| rect(p.frame)),
        "pending": rename.pending.as_ref().map(|p| json!({
            "object": object(&p.object),
            "text": p.text,
            "sending": p.sending(),
        })),
        "status": app.terminal.live.status,
    })
}

/// Publishes yebisu's side with the SDK's field of `window` in `qa.state`,
/// and records it (`rename-state`) when it changed, for rename-check.py.
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
pub(crate) fn publish(
    window: gpui::AnyWindowHandle,
    mut mine: Value,
    observation: &crate::observation::Observation,
) {
    if let Some(sdk) = herdr_uikit::native_inline_field_state(window) {
        mine["sdk"] = json!({
            "phase": sdk.phase,
            "field_first_responder": sdk.field_first_responder,
            "gpui_text_first_responder": sdk.gpui_text_first_responder,
            "gpui_drawing_first_responder": sdk.gpui_drawing_first_responder,
            "marked": sdk.marked,
            "primary_language": sdk.primary_language,
            "text": sdk.text,
            "frame": [sdk.frame.0, sdk.frame.1, sdk.frame.2, sdk.frame.3],
            "hidden": sdk.hidden,
            "hint": sdk.hint,
            "should_change_while_marked": sdk.should_change_while_marked,
            "late_drops": sdk.late_drops,
        });
    }
    crate::qa_ax::report("rename", || mine.clone());
    let changed = LAST_RECORD.with_borrow_mut(|last| {
        let changed = last.as_ref() != Some(&mine);
        if changed {
            *last = Some(mine.clone());
        }
        changed
    });
    if changed {
        let mut record = json!({"kind": "rename-state"});
        if let (Some(record), Some(fields)) = (record.as_object_mut(), mine.as_object()) {
            record.extend(fields.clone());
        }
        observation.record_without_waiting(record);
    }
}
