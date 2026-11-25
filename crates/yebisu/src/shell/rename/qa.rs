//! Simulator QA (issue #143): the rename's state in `qa.state.rename`,
//! yebisu's side and the SDK's field (`native_inline_field_state`), so a
//! SmokeQA test confirms by state rather than by pixels.
use super::{Phase, RenameAnchor, RenameObject};
use crate::app::Smoke;
use serde_json::{Value, json};
use sheprd_commands::Command;

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
        // S8, S10: the menus a rename starts from, and the palette's rows.
        "rail": app.actions.board_hidden,
        "rail_menu": app.shell.rail.menu_open(),
        "tab_menu": app.shell.rename.tab_menu.as_ref().map(|menu| json!({
            "core": menu.core,
            "rename_reason": app.tab_menu_rename(menu).1,
        })),
        "palette": app.actions.palette.then(|| palette_rows(app)),
    })
}

/// The palette's Rename rows: enabled, and the reason line under a disabled
/// one (`palette_reason`, what the row draws).
fn palette_rows(app: &Smoke) -> Value {
    let rows = crate::actions::catalog(&app.terminal)
        .into_iter()
        .filter(|a| {
            matches!(
                a.info.command,
                Command::RenameTab | Command::RenameWorkspace | Command::RenamePane
            )
        })
        .map(|a| {
            json!({
                "name": a.info.name,
                "label": a.label,
                "enabled": a.available,
                "reason": app.palette_reason(a.info.command, a.available),
            })
        })
        .collect::<Vec<_>>();
    json!(rows)
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
    // S7: once the field is open over an agent row, the wire peer adds an
    // agent above it (`YEBISU_RENAME_QA_INSERT`).
    if mine["phase"] == "Open" && mine["anchor"] == "AgentRow" {
        crate::mock_peer::request_agent_insert();
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
