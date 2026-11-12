//! Real SDK dispatch -> production Smoke guard -> private peer wire receipts.
//! Inject time; never sleep to age an event or pretend this is an OS response.
use super::*;
use gpui::{AppContext, TestAppContext};
use herdr_uikit::{Authorization, NotificationResponseFixture};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
const DEFAULT: &str = "com.apple.UNNotificationDefaultActionIdentifier";

#[gpui::test]
fn sdk_response_boundary_current_six_seconds_focuses_once_and_refuses_duplicate(
    cx: &mut TestAppContext,
) {
    exercise(cx, "current", 6, None, 1, "Stale");
}
#[gpui::test]
fn sdk_response_boundary_expired_original_event_never_focuses(cx: &mut TestAppContext) {
    exercise(cx, "expired", 301, None, 0, "Stale");
}
#[gpui::test]
fn sdk_response_boundary_disconnected_and_removed_tags_are_explained(cx: &mut TestAppContext) {
    exercise(cx, "disconnected", 6, None, 0, "Disconnected");
    exercise(cx, "removed", 6, None, 0, "Stale");
}
#[gpui::test]
fn sdk_response_boundary_stale_generation_boot_and_pane_never_focus(cx: &mut TestAppContext) {
    exercise(cx, "generation", 6, None, 0, "Stale");
    exercise(cx, "boot", 6, None, 0, "Stale");
    exercise(cx, "pane", 6, None, 0, "InvalidTarget");
}
#[gpui::test]
fn sdk_response_boundary_quiet_mute_and_actions_stay_distinct(cx: &mut TestAppContext) {
    exercise(cx, "quiet", 6, None, 0, "QuietHours");
    exercise(cx, "muted", 6, None, 0, "Muted");
    exercise(cx, "action", 6, Some("unsupported"), 0, "InvalidTarget");
}
#[gpui::test]
fn sdk_response_boundary_denied_is_unissued_and_has_no_wire(cx: &mut TestAppContext) {
    exercise(cx, "denied", 6, None, 0, "Permission(Denied)");
}

fn exercise(
    cx: &mut TestAppContext,
    case: &str,
    seconds: u64,
    action: Option<&str>,
    expected_focus: usize,
    expected_status: &str,
) {
    let view = cx.new(Smoke::new);
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("private peer: {e}"));
    let mut live = crate::live::Live::default();
    live.connected = true;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !live
        .snapshot
        .as_deref()
        .zip(live.surface.as_deref())
        .is_some_and(|(s, f)| crate::live::coherent(s, f))
    {
        live.apply(
            client
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|e| panic!("peer readiness: {e}")),
        );
    }
    let start = Instant::now();
    let age = Arc::new(AtomicU64::new(0));
    let display_age = age.clone();
    let sdk = NotificationResponseFixture::default();
    let issued = sdk.issue(
        case,
        start,
        if case == "denied" {
            Authorization::Denied
        } else {
            Authorization::Authorized
        },
        move || display_age.load(Ordering::Acquire) < 5,
    );
    let mut callbacks = 0;
    view.update(cx, |this, cx| {
        this.terminal.reconnect.target = "fixture".into();
        this.terminal.live = live.clone();
        *this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = live;
        this.terminal.link.handle = Some(client.handle.clone());
        let generation = this.terminal.link.generation;
        this.notifications.policy.route = Route::System;
        this.notifications.policy.sound = false;
        if case == "denied" {
            assert_eq!(
                issued,
                Err(herdr_uikit::NotificationError::Permission(
                    Authorization::Denied
                ))
            );
            this.notifications
                .fail(Failure::Permission(notifications::Authorization::Denied));
        } else {
            assert_eq!(issued, Ok(()));
            assert!(sdk.presents(case));
            this.notifications.posted.push_back(Posted {
                tag: case.into(),
                target: TargetContext {
                    host: "fixture".into(),
                    generation,
                    boot: "boot-v1".into(),
                    pane: Some("w1:p3".into()),
                    tab: None,
                    workspace: None,
                },
                received: start,
            });
        }
        match case {
            "disconnected" => this
                .terminal
                .link
                .notification_active
                .store(false, Ordering::Release),
            "removed" => {
                this.notifications.posted.clear();
                sdk.remove(case);
            }
            "generation" => this.terminal.link.generation += 1,
            "boot" | "pane" => {
                let posted = this
                    .notifications
                    .posted
                    .front_mut()
                    .unwrap_or_else(|| panic!("record"));
                if case == "boot" {
                    posted.target.boot = "old-boot".into();
                } else {
                    posted.target.pane = Some("missing-pane".into());
                }
            }
            "quiet" => this.notifications.policy.quiet = Some((0, 60)),
            "muted" => this.notifications.policy.muted = true,
            _ => (),
        }
        let before = this.terminal.link.generation;
        age.store(seconds, Ordering::Release);
        assert!(
            !sdk.presents(case),
            "late willPresent must remain suppressed"
        );
        let delivered = sdk.respond(
            case,
            action.unwrap_or(DEFAULT),
            start + Duration::from_secs(seconds),
            |r| {
                callbacks += 1;
                this.open_notification_at(
                    &r.tag,
                    r.action_id.is_some(),
                    start + Duration::from_secs(seconds),
                    30,
                    cx,
                );
            },
        );
        assert_eq!(
            delivered,
            case != "denied",
            "D5 SDK response must reach consumer guard"
        );
        if expected_focus == 1 {
            assert!(
                this.terminal.navigating.is_some(),
                "six-second response must enqueue focus"
            );
            assert!(sdk.respond(
                case,
                DEFAULT,
                start + Duration::from_secs(seconds + 1),
                |r| {
                    callbacks += 1;
                    this.open_notification_at(
                        &r.tag,
                        false,
                        start + Duration::from_secs(seconds + 1),
                        30,
                        cx,
                    );
                }
            ));
        } else {
            assert!(this.terminal.navigating.is_none());
        }
        assert!(this.notifications.posted.is_empty());
        assert!(
            this.notifications.status.contains(expected_status),
            "{}",
            this.notifications.status
        );
        assert_eq!(
            this.terminal.link.generation, before,
            "response must never reconnect"
        );
        assert!(!this.setup.busy);
        assert!(!sdk.presents(case));
    });
    assert_eq!(
        callbacks,
        if case == "denied" {
            0
        } else if expected_focus == 1 {
            2
        } else {
            1
        }
    );
    // A FIFO wire barrier proves no extra focus/input preceded it; no timing sleep.
    client
        .handle
        .send_input(
            "boot-v1",
            if expected_focus == 1 {
                "w1:p3"
            } else {
                "w1:p1"
            },
            [ClientPaneInputEvent::Paste("d5-barrier".into())],
        )
        .unwrap_or_else(|e| panic!("barrier: {e}"));
    let mut focuses = 0;
    loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire barrier: {e}"));
        match message {
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let request: serde_json::Value =
                    serde_json::from_str(&request).unwrap_or_else(|e| panic!("{e}"));
                assert_eq!(request["method"], "pane.focus", "unexpected wire API");
                assert_eq!(request["params"]["pane_id"], "w1:p3");
                focuses += 1;
            }
            ClientMessage::ClientShellPaneInput { events, .. } => {
                assert_eq!(events.len(), 1);
                assert!(
                    matches!(&events[0], ClientPaneInputEvent::Paste(text) if text == "d5-barrier")
                );
                break;
            }
            other => panic!("unexpected wire: {other:?}"),
        }
    }
    assert_eq!(focuses, expected_focus, "focus count at receiving peer");
    client.handle.disconnect();
}
