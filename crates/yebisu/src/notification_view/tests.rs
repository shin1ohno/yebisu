//! Actual Smoke/Link adapters, never fake OS delivery receipts.
use super::*;
use gpui::{AppContext, TestAppContext};
fn target() -> TargetContext {
    TargetContext {
        host: "fixture@private-peer".into(),
        generation: 1,
        boot: "fixture-boot".into(),
        pane: Some("fixture-pane".into()),
        tab: None,
        workspace: None,
    }
}
#[gpui::test]
fn stale_response_and_actions_cannot_reconnect_or_replay(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.terminal.reconnect.target = "fixture@private-peer".into();
        let generation = this.terminal.link.generation;
        this.notifications.posted.push_back(Posted {
            tag: "old".into(),
            target: target(),
            received: Instant::now(),
        });
        this.open_notification("old", false, cx);
        assert!(this.notifications.posted.is_empty());
        assert!(this.notifications.status.contains("Disconnected"));
        assert!(!this.terminal.open);
        assert!(!this.setup.busy);
        assert!(this.terminal.link.handle.is_none());
        assert_eq!(this.terminal.link.generation, generation);
        this.open_notification("old", false, cx);
        assert!(this.notifications.status.contains("Stale"));
        this.notifications.posted.push_back(Posted {
            tag: "action".into(),
            target: target(),
            received: Instant::now(),
        });
        this.open_notification("action", true, cx);
        assert!(this.notifications.status.contains("InvalidTarget"));
        assert!(this.terminal.link.handle.is_none());
        assert_eq!(this.terminal.link.generation, generation);
    });
}
#[test]
fn pause_invalidates_proof_before_reader_shutdown() {
    let mut link = crate::link::Link::default();
    let old = link.notification_active.clone();
    assert!(old.load(std::sync::atomic::Ordering::Acquire));
    link.pause();
    assert!(!old.load(std::sync::atomic::Ordering::Acquire));
    assert!(
        link.notification_active
            .load(std::sync::atomic::Ordering::Acquire)
    );
    let latest = link.notification_active.clone();
    drop(link);
    assert!(!latest.load(std::sync::atomic::Ordering::Acquire));
}
#[test]
fn inactive_removes_foreground_effects_but_background_invalidates_taps() {
    let mut state = State::default();
    state.posted.push_back(Posted {
        tag: "current".into(),
        target: target(),
        received: Instant::now(),
    });
    state.toast = Some(Toast {
        tag: "current".into(),
        title: "Agent finished",
        expires: Instant::now() + Duration::from_secs(5),
    });
    state.pause_foreground();
    assert!(state.toast.is_none());
    assert_eq!(state.posted.len(), 1);
    state.invalidate();
    assert!(state.posted.is_empty());
    assert!(state.waiting.is_empty());
}
#[test]
fn old_submission_completion_cannot_claim_success_after_invalidation() {
    let mut state = State::default();
    let old = state.epoch;
    state.invalidate();
    state
        .sender
        .try_send((old, Ok(())))
        .unwrap_or_else(|e| panic!("fixture: {e}"));
    assert!(!state.poll());
    assert!(state.status.is_empty());
}
