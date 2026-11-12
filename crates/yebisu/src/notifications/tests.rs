use super::*;
use crate::{link::Link, mock_peer};
use sheprd_client::{
    ClientEvent,
    protocol::{ClientShellSnapshot, ServerMessage},
};
use std::sync::Arc;
fn ready() -> Live {
    let snapshot: ClientShellSnapshot =
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"));
    let mut live = Live::default();
    live.connected = true;
    live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    live
}
fn event() -> SemanticNotification {
    SemanticNotification {
        kind: Kind::NeedsAttention,
        title: "secret title".into(),
        body: Some("secret body".into()),
        sound: Some(Sound::Request),
        agent: None,
        workspace_id: None,
        tab_id: None,
        pane_id: Some("w1:p1".into()),
        position: None,
    }
}
fn valid_event(live: &Live) -> SemanticNotification {
    let mut event = event();
    event.pane_id = live
        .snapshot
        .as_ref()
        .and_then(|s| s.panes.first().map(|p| p.pane_id.clone()));
    event
}
#[test]
fn actual_message_adapter_drains_once_and_drops_disconnected() {
    let link = Link::default();
    {
        let mut inbox = link.inbox.lock().unwrap_or_else(|e| e.into_inner());
        *inbox = ready();
        let event = valid_event(&inbox);
        inbox.apply(ClientEvent::Message(ServerMessage::SemanticNotification(
            event.clone(),
        )));
        inbox.apply(ClientEvent::Message(ServerMessage::SemanticNotification(
            event,
        )));
    }
    let update = link
        .take_update()
        .unwrap_or_else(|| panic!("missing update"));
    assert_eq!(update.notifications.pending.len(), 1);
    assert_eq!(update.notifications.failure, Some(Failure::Duplicate));
    assert!(link.take_update().is_none());
    {
        let mut inbox = link.inbox.lock().unwrap_or_else(|e| e.into_inner());
        let event = valid_event(&inbox);
        inbox.apply(ClientEvent::Disconnected {
            reason: "test".into(),
            ssh: None,
        });
        inbox.apply(ClientEvent::Message(ServerMessage::SemanticNotification(
            event,
        )));
    }
    let update = link
        .take_update()
        .unwrap_or_else(|| panic!("missing update"));
    assert!(update.notifications.pending.is_empty());
    assert_eq!(update.notifications.failure, Some(Failure::Disconnected));
}
#[test]
fn current_target_validation_rejects_host_boot_generation_and_incoherence() {
    let mut live = ready();
    let event = Event::capture(&live, valid_event(&live), Instant::now())
        .unwrap_or_else(|e| panic!("capture: {e:?}"));
    let context = event.context("fixture".into(), 8);
    assert_eq!(context.validate(&live, "fixture", 8), Ok(()));
    assert_eq!(context.validate(&live, "other", 8), Err(Failure::Stale));
    assert_eq!(context.validate(&live, "fixture", 9), Err(Failure::Stale));
    let mut changed = context.clone();
    changed.boot = "other".into();
    assert_eq!(changed.validate(&live, "fixture", 8), Err(Failure::Stale));
    changed = context.clone();
    changed.pane = Some("absent".into());
    assert_eq!(
        changed.validate(&live, "fixture", 8),
        Err(Failure::InvalidTarget)
    );
    live.pending_snapshot = live.snapshot.clone();
    assert_eq!(
        context.validate(&live, "fixture", 8),
        Err(Failure::Incoherent)
    );
}
#[test]
fn payload_is_redacted_and_identifiers_bounded() {
    let live = ready();
    let captured = Event::capture(&live, valid_event(&live), Instant::now())
        .unwrap_or_else(|e| panic!("capture: {e:?}"));
    assert_eq!(captured.title(), "Agent needs attention");
    let mut invalid = valid_event(&live);
    invalid.pane_id = Some("x".repeat(129));
    assert!(matches!(
        Event::capture(&live, invalid, Instant::now()),
        Err(Failure::InvalidTarget)
    ));
}
#[test]
fn overflow_and_policy_boundaries_are_explicit() {
    let live = ready();
    let mut inbox = Inbox::default();
    let now = Instant::now();
    for index in 0..LIMIT + 1 {
        let at = now + Duration::from_secs(index as u64 * 3);
        inbox.push(Event::capture(&live, valid_event(&live), at), at);
    }
    assert_eq!(inbox.pending.len(), LIMIT);
    assert_eq!(inbox.failure, Some(Failure::Overflow));
    let mut policy = Policy::default();
    assert_eq!(policy.route, Route::Off);
    assert_eq!(policy.volume, 50);
    policy.quiet = Some((22 * 60, 8 * 60));
    assert_eq!(policy.suppression(23 * 60), Some(Failure::QuietHours));
    assert_eq!(policy.suppression(8 * 60), None);
    policy.quiet = Some((3, 3));
    assert_eq!(policy.suppression(3), None);
    policy.muted = true;
    assert_eq!(policy.suppression(8 * 60), Some(Failure::Muted));
}

#[test]
fn persistence_preserves_defaults_and_rejects_invalid_ranges() {
    let policy = Policy::default();
    assert_eq!(Policy::decode(None).ok(), Some(policy.clone()));
    assert_eq!(
        Policy::decode(Some(&policy.encode())).ok(),
        Some(policy.clone())
    );
    let mut invalid = policy.encode();
    invalid["volume"] = serde_json::json!(101);
    assert!(Policy::decode(Some(&invalid)).is_err());
    invalid = policy.encode();
    invalid["quiet"] = serde_json::json!([0, 1440]);
    assert!(Policy::decode(Some(&invalid)).is_err());
    for state in [
        Authorization::NotDetermined,
        Authorization::Denied,
        Authorization::Authorized,
        Authorization::Provisional,
        Authorization::Ephemeral,
        Authorization::Unknown,
    ] {
        assert!(format!("{:?}", Failure::Permission(state)).starts_with("Permission"));
    }
}

#[test]
fn editing_sound_before_load_preserves_other_notification_preferences() {
    let before = Policy::default();
    let mut draft = before.clone();
    draft.sound = false;
    let mut edits = Edits::default();
    edits.track(&before, &draft);
    let loaded = Policy {
        route: Route::System,
        volume: 25,
        muted: true,
        quiet: Some((1320, 480)),
        ..before
    };
    edits.merge(&mut draft, loaded.clone());
    assert!(!draft.sound);
    assert_eq!(draft.route, loaded.route);
    assert_eq!(draft.volume, loaded.volume);
    assert_eq!(draft.muted, loaded.muted);
    assert_eq!(draft.quiet, loaded.quiet);
}
