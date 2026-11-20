use super::*;
use sheprd_core_client::wire::Terminal;

#[test]
fn core_status_maps_to_sidebar_status() {
    let cases = [
        (Some("blocked"), SidebarStatus::NeedsYou),
        (Some("error"), SidebarStatus::Error),
        (Some("working"), SidebarStatus::Working),
        (Some("done"), SidebarStatus::Done),
        (Some("idle"), SidebarStatus::Idle),
        (Some("thinking-hard"), SidebarStatus::Unknown),
        (None, SidebarStatus::NoStatus),
    ];
    for (state, expected) in cases {
        let status = raw("t1", state);
        assert_eq!(core_status_to_sidebar(Some(&status)), expected, "{state:?}");
    }
    // A terminal missing from the list reported nothing either.
    assert_eq!(core_status_to_sidebar(None), SidebarStatus::NoStatus);
    // Error is its own value, never folded into Needs you (F1).
    assert_ne!(
        core_status_to_sidebar(Some(&raw("t1", Some("error")))),
        SidebarStatus::NeedsYou
    );
    // All seven sidebar states are reachable from Herdr 2.
    let mut reached = cases.iter().map(|(_, s)| *s).collect::<Vec<_>>();
    reached.sort();
    assert_eq!(reached, SidebarStatus::ALL);
}

#[test]
fn core_status_reads_a_listed_terminal() {
    let terminal: Terminal = serde_json::from_value(serde_json::json!({
        "address": "t7", "argv": ["sh"], "cwd": "/tmp", "state": "live", "created_ms": 1,
        "status": {"app": "claude", "state": "error", "title": "review"},
    }))
    .unwrap_or_else(|e| panic!("wire terminal: {e}"));
    let status = CoreStatus::from(&terminal);
    assert_eq!(status.address, "t7");
    assert_eq!(core_status_to_sidebar(Some(&status)), SidebarStatus::Error);
}

#[test]
fn core_qa_mock_has_error_and_status_less_terminals() {
    let status = |id: &str| CoreStatus {
        address: id.into(),
        status: crate::core_mock::qa_status(id).map(|value| {
            serde_json::from_value(value).unwrap_or_else(|e| panic!("mock status {id}: {e}"))
        }),
    };
    assert_eq!(
        core_status_to_sidebar(Some(&status("t1"))),
        SidebarStatus::NoStatus
    );
    assert_eq!(
        core_status_to_sidebar(Some(&status("t2"))),
        SidebarStatus::Error
    );
    assert_eq!(
        core_status_to_sidebar(Some(&status("t3"))),
        SidebarStatus::NoStatus
    );
}
