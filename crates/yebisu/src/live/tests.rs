use super::*;
use crate::{
    link::{HeldInput, Link},
    mock_peer,
    terminal::InputTarget,
};
use std::time::{Duration, Instant};
fn snapshot(revision: u64) -> Arc<ClientShellSnapshot> {
    let mut s: ClientShellSnapshot =
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"));
    s.revision = revision;
    Arc::new(s)
}
fn surface(revision: u64) -> Arc<PaneSurfaceFrame> {
    Arc::new(mock_peer::surface(revision))
}
fn ready() -> Live {
    let mut s = Live::default();
    s.apply(ClientEvent::Snapshot(snapshot(7)));
    s.apply(ClientEvent::Surface(surface(7)));
    s
}
fn pair(s: &Live, revision: u64) {
    let snap = s
        .snapshot
        .as_ref()
        .unwrap_or_else(|| panic!("missing snapshot"));
    let surf = s
        .surface
        .as_ref()
        .unwrap_or_else(|| panic!("missing surface"));
    assert!(coherent(snap, surf));
    assert_eq!(snap.revision, revision);
}
#[test]
fn future_surface_then_snapshot() {
    let mut s = ready();
    s.apply(ClientEvent::Surface(surface(8)));
    pair(&s, 7);
    s.apply(ClientEvent::Snapshot(snapshot(8)));
    pair(&s, 8);
}
#[test]
fn metadata_only_revision_keeps_old_coherent_pair() {
    let mut s = ready();
    s.apply(ClientEvent::Snapshot(snapshot(8)));
    pair(&s, 7);
    assert_eq!(s.pending_snapshot.as_ref().map(|s| s.revision), Some(8));
    s.apply(ClientEvent::Surface(surface(8)));
    pair(&s, 8);
    assert!(s.pending_snapshot.is_none());
}
#[test]
fn old_boot_and_old_revisions_cannot_mix() {
    let mut s = ready();
    let mut stale = (*surface(7)).clone();
    stale.boot_id = "retired".into();
    s.apply(ClientEvent::Surface(Arc::new(stale)));
    pair(&s, 7);
    let mut old = (*snapshot(99)).clone();
    old.boot_id = "retired".into();
    s.apply(ClientEvent::Snapshot(Arc::new(old)));
    pair(&s, 7);
    s.apply(ClientEvent::Snapshot(snapshot(6)));
    s.apply(ClientEvent::Surface(surface(6)));
    pair(&s, 7);
    let mut regression = (*surface(7)).clone();
    regression.surface_revision = 0;
    s.apply(ClientEvent::Surface(Arc::new(regression)));
    assert_eq!(s.surface.as_ref().map(|s| s.surface_revision), Some(1));
}
#[test]
fn input_projection_change_invalidates_display() {
    let mut s = ready();
    let mut changed = (*snapshot(8)).clone();
    changed.focused_pane_id = Some("other".into());
    s.apply(ClientEvent::Snapshot(Arc::new(changed)));
    assert!(s.surface.is_none());
}
#[test]
fn clipboard_is_bounded_utf8_and_drained_once() {
    let link = Link::default();
    {
        let mut live = link.inbox.lock().unwrap_or_else(|e| e.into_inner());
        for _ in 0..20 {
            live.apply(ClientEvent::Message(ServerMessage::Clipboard {
                data: "dGVzdA==".into(),
            }));
        }
        live.apply(ClientEvent::Message(ServerMessage::Clipboard {
            data: "/w==".into(),
        }));
    }
    let first = link
        .take_update()
        .unwrap_or_else(|| panic!("missing update"));
    assert_eq!(first.clipboard.len(), 8);
    assert!(first.clipboard.iter().all(|s| s == "test"));
    assert!(link.take_update().is_none());
    link.inbox
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .apply(ClientEvent::Message(
            ServerMessage::ClientShellKeyboardReportAll { enabled: true },
        ));
    let next = link
        .take_update()
        .unwrap_or_else(|| panic!("missing keyboard update"));
    assert!(next.keyboard_report_all);
    assert!(next.clipboard.is_empty());
}
#[test]
fn startup_failure_is_visible_and_drained() {
    let mut link = Link::default();
    let g = link.reset();
    link.install(g, Err(crate::Error::Ssh("startup fixture".into())));
    let update = link
        .take_update()
        .unwrap_or_else(|| panic!("missing failure"));
    assert!(update.status.contains("startup fixture"));
    assert!(link.drained.load(std::sync::atomic::Ordering::Acquire));
}
#[test]
fn reconnect_isolates_old_reader() {
    let mut link = Link::default();
    let old_generation = link.reset();
    let old = link.inbox.clone();
    let old_drained = link.drained.clone();
    let new_generation = link.reset();
    assert_ne!(old_generation, new_generation);
    old.lock()
        .unwrap_or_else(|e| e.into_inner())
        .apply(ClientEvent::Snapshot(snapshot(7)));
    old_drained.store(true, std::sync::atomic::Ordering::Release);
    assert!(!link.current(old_generation));
    assert!(link.current(new_generation));
    assert!(link.take_update().is_some_and(|s| s.snapshot.is_none()));
    assert!(!link.drained.load(std::sync::atomic::Ordering::Acquire));
    link.install(
        old_generation,
        Err(crate::Error::Ssh("late old startup".into())),
    );
    assert!(link.take_update().is_none());
}
#[test]
fn ui_try_lock_never_waits_for_reader() {
    let link = Link::default();
    let _held = link.inbox.lock().unwrap_or_else(|e| e.into_inner());
    assert!(link.take_update().is_none());
}
#[test]
fn bounded_input_has_boot_and_popup_fence() {
    let pane = InputTarget::Pane("p".into());
    let mut held = HeldInput::new("boot".into(), pane.clone());
    for n in 0..64 {
        assert!(
            held.push(sheprd_client::protocol::ClientPaneInputEvent::TextCommit(
                n.to_string()
            ))
        );
    }
    assert!(
        !held.push(sheprd_client::protocol::ClientPaneInputEvent::TextCommit(
            "overflow".into()
        ))
    );
    assert!(
        matches!(held.pop("boot",&pane),Some(sheprd_client::protocol::ClientPaneInputEvent::TextCommit(s)) if s=="0")
    );
    assert!(held.pop("boot", &InputTarget::Popup("p".into())).is_none());
    assert!(held.pop("boot", &pane).is_none());
    assert!(
        held.push(sheprd_client::protocol::ClientPaneInputEvent::TextCommit(
            "old".into()
        ))
    );
    assert!(held.pop("new boot", &pane).is_none());
}
#[test]
fn private_wire_peer_reaches_the_real_reader() {
    let mut link = Link::default();
    let g = link.reset();
    link.install(g, mock_peer::connect());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(update) = link.take_update()
            && update.surface.is_some()
        {
            pair(&update, 7);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "wire peer did not deliver surface"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn receiver_congestion_drains_while_ui_is_busy() {
    let mut link = Link::default();
    let g = link.reset();
    link.install(g, mock_peer::connect_burst(600));
    // No take_update until the reader has consumed more than the bounded event
    // queue capacity (8 at the pinned client revision). Poll only the receiving mailbox, as an observer.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let revision = link
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .surface
            .as_ref()
            .map(|s| s.surface_revision);
        if revision == Some(600) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "single reader stopped while UI busy: {revision:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let update = link
        .take_update()
        .unwrap_or_else(|| panic!("no latest mailbox"));
    pair(&update, 7);
    assert_eq!(
        update.surface.as_ref().map(|s| s.surface_revision),
        Some(600)
    );
}

#[test]
fn p2_reply_variants_remain_visible() {
    let mut setup = crate::setup::Setup::default();
    setup.apply(Ok(crate::setup::Reply::Loaded("host".into(), "pin".into())));
    assert_eq!(setup.target, "host");
    setup.apply(Ok(crate::setup::Reply::Saved(
        crate::hosts::SavedHosts {
            targets: vec!["saved".into()],
            selected: "saved".into(),
        },
        "pin2".into(),
    )));
    assert_eq!(setup.target, "saved");
    setup.apply(Ok(crate::setup::Reply::Authenticated(
        crate::hosts::SavedHosts::default(),
        "authenticated".into(),
    )));
    assert_eq!(setup.status, "authenticated");
}

#[test]
fn mismatch_survives_disconnect_and_response_requires_correlation() {
    let mut s = ready();
    s.pending_request = Some("wanted".into());
    s.apply(ClientEvent::Response {
        request_id: "stale".into(),
        response: serde_json::Value::Null,
    });
    assert_eq!(s.pending_request.as_deref(), Some("wanted"));
    s.apply(ClientEvent::Response {
        request_id: "wanted".into(),
        response: serde_json::Value::Null,
    });
    assert!(s.pending_request.is_none());
    s.apply(ClientEvent::SurfaceImages(Arc::default()));
    pair(&s, 7);
    s.apply(ClientEvent::CommandRejected {
        request_id: None,
        reason: sheprd_client::Error::Disconnected,
    });
    assert!(!s.status.is_empty());
    s.apply(ClientEvent::VersionMismatch(
        VersionMismatch::DaemonOutdated {
            server_version: Some("0.8".into()),
        },
    ));
    let status = s.status.clone();
    assert!(s.surface.is_none());
    s.apply(ClientEvent::Disconnected {
        reason: "closed".into(),
        ssh: None,
    });
    assert_eq!(s.status, status);
    assert!(s.version_mismatch.get().is_some());
}

#[test]
fn viewport_follows_rotation_density_and_protocol_limits() {
    let portrait = crate::terminal::viewport(1000., 1300., 8., 20.);
    let landscape = crate::terminal::viewport(1300., 1000., 8., 20.);
    assert_eq!((portrait.cols, portrait.rows), (125, 65));
    assert_eq!((landscape.cols, landscape.rows), (162, 50));
    let compact = crate::terminal::viewport(1000., 1300., 7., 17.);
    assert_eq!((compact.cols, compact.rows), (142, 76));
    let huge = crate::terminal::viewport(100000., 100000., 1., 1.);
    assert!(u32::from(huge.cols) * u32::from(huge.rows) <= 1_000_000);
}

#[test]
fn image_catalog_waits_for_coherent_surface_and_clears_on_disconnect() {
    let mut live = ready();
    let images = Arc::new(sheprd_client::SurfaceImages::default());
    live.apply(ClientEvent::SurfaceImages(images.clone()));
    assert!(!Arc::ptr_eq(&live.surface_images, &images));
    live.apply(ClientEvent::Surface(surface(8)));
    assert!(!Arc::ptr_eq(&live.surface_images, &images));
    live.apply(ClientEvent::Snapshot(snapshot(8)));
    assert!(Arc::ptr_eq(&live.surface_images, &images));
    live.apply(ClientEvent::Disconnected {
        reason: "test".into(),
        ssh: None,
    });
    assert!(!Arc::ptr_eq(&live.surface_images, &images));
    assert!(live.pending_surface_images.is_none());
}

#[test]
fn rejected_paste_is_visible_without_changing_input_target() {
    let mut live = ready();
    live.connected = true;
    let target = crate::terminal_input::destination(&live, 1);
    live.apply(ClientEvent::CommandRejected {
        request_id: None,
        reason: sheprd_client::clipboard::Error::Cancelled.into(),
    });
    assert!(!live.status.is_empty());
    assert_eq!(crate::terminal_input::destination(&live, 1), target);
}
