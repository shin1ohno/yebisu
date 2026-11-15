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

#[gpui::test]
fn modal_response_is_consumed_without_focus_command_or_key_owner_change(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.settings_modal = true;
        this.terminal.open = true;
        let generation = this.terminal.link.generation;
        this.notifications.posted.push_back(Posted {
            tag: "modal".into(),
            target: target(),
            received: Instant::now(),
        });
        this.open_notification("modal", false, cx);
        assert!(this.settings_modal);
        assert!(this.terminal.open);
        assert!(this.notifications.posted.is_empty());
        assert!(this.notifications.status.contains("Busy"));
        assert_eq!(this.terminal.link.generation, generation);
        assert!(this.terminal.link.handle.is_none());
    });
}

const HOST: &str = "fixture@private-peer";

fn ready_live() -> crate::live::Live {
    let snapshot: sheprd_client::protocol::ClientShellSnapshot =
        serde_json::from_str(crate::mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"));
    let mut live = crate::live::Live::default();
    live.connected = true;
    live.apply(sheprd_client::ClientEvent::Snapshot(std::sync::Arc::new(
        snapshot,
    )));
    live.apply(sheprd_client::ClientEvent::Surface(std::sync::Arc::new(
        crate::mock_peer::surface(7),
    )));
    live
}

fn semantic(live: &crate::live::Live) -> sheprd_client::protocol::SemanticNotification {
    use sheprd_client::protocol::{SemanticNotification, SemanticNotificationKind as Kind};
    SemanticNotification {
        kind: Kind::Finished,
        title: "redacted".into(),
        body: None,
        sound: None,
        agent: None,
        workspace_id: None,
        tab_id: None,
        pane_id: live
            .snapshot
            .as_ref()
            .and_then(|s| s.panes.first().map(|p| p.pane_id.clone())),
        position: None,
    }
}

/// Installs a coherent reader state for HOST and returns a target bound to it.
fn current_target(this: &mut Smoke) -> TargetContext {
    this.terminal.reconnect.target = HOST.into();
    let live = ready_live();
    let target = Event::capture(&live, semantic(&live), Instant::now())
        .unwrap_or_else(|e| panic!("capture: {e:?}"))
        .context(HOST.into(), this.terminal.link.generation);
    *this
        .terminal
        .link
        .inbox
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = live;
    target
}

/// Holds the reader's inbox lock on another thread while `during` runs.
fn with_reader_lock<T>(
    inbox: &std::sync::Arc<std::sync::Mutex<crate::live::Live>>,
    during: impl FnOnce() -> T,
) -> T {
    let (locked, wait_locked) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let _guard = inbox.lock().unwrap_or_else(|e| e.into_inner());
            let _ = locked.send(());
            let _ = wait_release.recv();
        });
        wait_locked
            .recv()
            .unwrap_or_else(|e| panic!("reader thread did not take the lock: {e}"));
        let result = during();
        let _ = release.send(());
        result
    })
}

#[gpui::test]
fn reader_lock_contention_never_cancels_a_valid_posted_notification(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        let target = current_target(this);
        this.notifications.posted.push_back(Posted {
            tag: "current".into(),
            target,
            received: Instant::now(),
        });
        let inbox = this.terminal.link.inbox.clone();
        with_reader_lock(&inbox, || {
            for _ in 0..50 {
                this.collect_notifications(cx);
            }
        });
        assert_eq!(this.notifications.posted.len(), 1);
        assert!(!this.notifications.status.contains("Stale"));
        this.collect_notifications(cx);
        assert_eq!(this.notifications.posted.len(), 1);
        // A decided mismatch is still cancelled, so the fix is not a blanket skip.
        this.notifications.posted[0].target.boot = "other-boot".into();
        this.collect_notifications(cx);
        assert!(this.notifications.posted.is_empty());
        assert!(this.notifications.status.contains("Stale"));
    });
}

#[gpui::test]
fn reader_lock_contention_defers_a_waiting_event_instead_of_dropping_it(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        let target = current_target(this);
        let live = ready_live();
        let aged = Instant::now()
            .checked_sub(Duration::from_secs(2))
            .unwrap_or_else(|| panic!("clock too young for the fixture"));
        let event = Event::capture(&live, semantic(&live), aged)
            .unwrap_or_else(|e| panic!("capture: {e:?}"));
        this.notifications.waiting.push_back((event, target));
        let inbox = this.terminal.link.inbox.clone();
        with_reader_lock(&inbox, || this.collect_notifications(cx));
        assert_eq!(this.notifications.waiting.len(), 1);
        assert!(!this.notifications.status.contains("Busy"));
        // Once the reader is free the event is decided; the default route is Off.
        this.collect_notifications(cx);
        assert!(this.notifications.waiting.is_empty());
        assert!(this.notifications.status.contains("Disabled"));
    });
}

#[gpui::test]
fn terminal_closed_to_the_hosts_list_still_receives_current_host_notifications(
    cx: &mut TestAppContext,
) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        current_target(this);
        this.terminal.open = false;
        {
            let mut inbox = this
                .terminal
                .link
                .inbox
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let event = semantic(&inbox);
            inbox.apply(sheprd_client::ClientEvent::Message(
                sheprd_client::protocol::ServerMessage::SemanticNotification(event),
            ));
        }
        this.poll_terminal(cx);
        assert_eq!(this.terminal.live.notifications.pending.len(), 1);
        this.collect_notifications(cx);
        assert_eq!(this.notifications.waiting.len(), 1);
        assert!(this.terminal.live.notifications.pending.is_empty());
    });
}

#[gpui::test]
fn refused_tap_keeps_the_open_terminal_and_busy_keeps_the_single_use_tag(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        let target = current_target(this);
        this.terminal.open = true;
        let mut elsewhere = target.clone();
        elsewhere.host = "other@private-peer".into();
        for (tag, target) in [("gone", elsewhere), ("busy", target)] {
            this.notifications.posted.push_back(Posted {
                tag: tag.into(),
                target,
                received: Instant::now(),
            });
        }
        this.open_notification("gone", false, cx);
        assert!(
            this.terminal.open,
            "a refused tap must not close the terminal"
        );
        assert!(this.notifications.status.contains("Disconnected"));
        assert!(this.notifications.posted.iter().all(|p| p.tag != "gone"));
        let inbox = this.terminal.link.inbox.clone();
        with_reader_lock(&inbox, || this.open_notification("busy", false, cx));
        assert!(this.terminal.open);
        assert!(this.notifications.status.contains("Busy"));
        assert!(
            this.notifications.posted.iter().any(|p| p.tag == "busy"),
            "a transient Busy must not spend the tag"
        );
        this.open_notification("busy", false, cx);
        assert!(this.notifications.posted.iter().all(|p| p.tag != "busy"));
    });
}

#[gpui::test]
fn lifecycle_phases_gate_foreground_effects_and_background_clears_pending(cx: &mut TestAppContext) {
    use gpui::AppLifecyclePhase as Phase;
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        let target = current_target(this);
        let live = ready_live();
        let event = Event::capture(&live, semantic(&live), Instant::now())
            .unwrap_or_else(|e| panic!("capture: {e:?}"));
        this.terminal
            .live
            .notifications
            .push(Ok(event), Instant::now());
        this.notifications.posted.push_back(Posted {
            tag: "current".into(),
            target,
            received: Instant::now(),
        });
        this.notifications.toast = Some(Toast {
            tag: "current".into(),
            title: "Agent finished",
            expires: Instant::now() + Duration::from_secs(5),
        });
        this.notification_lifecycle(Phase::Inactive);
        assert!(!this.notifications.active);
        assert!(this.notifications.toast.is_none());
        assert_eq!(this.notifications.posted.len(), 1);
        assert_eq!(this.terminal.live.notifications.pending.len(), 1);
        this.notification_lifecycle(Phase::Active);
        assert!(this.notifications.active);
        this.notification_lifecycle(Phase::Foreground);
        assert!(!this.notifications.active);
        this.notification_lifecycle(Phase::Background);
        assert!(!this.notifications.active);
        assert!(this.notifications.posted.is_empty());
        assert!(this.terminal.live.notifications.pending.is_empty());
        this.notifications.active = true;
        this.notification_lifecycle(Phase::Disconnected);
        assert!(!this.notifications.active);
    });
}
