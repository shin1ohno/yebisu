use super::*;
use crate::shell::sidebar::target::resolve_target;
use std::collections::BTreeMap;

fn snapshot() -> sheprd_client::protocol::ClientShellSnapshot {
    serde_json::from_str(crate::mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"))
}

#[test]
fn connection_key_is_host_and_kind() {
    let one = ConnectionKey::new("pro-dev", HerdrKind::Herdr1);
    let two = ConnectionKey::new("pro-dev", HerdrKind::Herdr2);
    assert_ne!(
        one, two,
        "one host holds a Herdr 1 and a Herdr 2 connection"
    );
    assert_eq!(two, ConnectionKey::new("pro-dev", HerdrKind::Herdr2));
    assert!(one < two, "Herdr 1 first on the same host");
    let mut connections = BTreeMap::new();
    connections.insert(two.clone(), "core");
    connections.insert(one.clone(), "shell");
    connections.insert(ConnectionKey::new("mini", HerdrKind::Herdr2), "core");
    assert_eq!(connections.len(), 3);
    assert_eq!(connections.get(&one), Some(&"shell"));
    // A Herdr 2 disconnect forgets only that connection's receive order.
    let state = SidebarState::default();
    let observed = |pane: &str| crate::shell::sidebar::Observed {
        pane: pane.into(),
        status: SidebarStatus::Working,
        seq: 1,
    };
    state.observe("pro-dev", HerdrKind::Herdr1, "b1", &[observed("p1")]);
    state.observe(
        "pro-dev",
        HerdrKind::Herdr1,
        "b1",
        &[observed("p1"), observed("p2")],
    );
    let herdr1 = AgentIdentity {
        host: "pro-dev".into(),
        kind: HerdrKind::Herdr1,
        pane: "p2".into(),
    };
    let before = state.received(&herdr1);
    assert!(before > 0);
    let _ = host_entry(
        "pro-dev",
        HostLink::Offline(crate::shell::sidebar::model::OfflineReason::NotConnected),
        &[],
        &state,
    );
    assert_eq!(state.received(&herdr1), before);
}

#[test]
fn notification_context_keeps_herdr_kind() {
    let snap = snapshot();
    let pane = snap.panes[0].pane_id.clone();
    let target = Context {
        host: "pro-dev".into(),
        generation: 1,
        boot: snap.boot_id.clone(),
        pane: Some(pane.clone()),
        tab: None,
        workspace: None,
    };
    let on_two = notification_agent(&target, &ConnectionKey::new("pro-dev", HerdrKind::Herdr2))
        .unwrap_or_else(|| panic!("a pane notification has an agent"));
    assert_eq!(on_two.kind, HerdrKind::Herdr2);
    assert_eq!(on_two.pane, pane);
    // A Herdr 2 terminal never resolves to the Herdr 1 pane of that host.
    assert_eq!(
        resolve_target(&target, &on_two, ("pro-dev", HerdrKind::Herdr1), &snap),
        None
    );
    // Positive control: the same notification from Herdr 1 resolves there.
    let on_one = notification_agent(&target, &ConnectionKey::new("pro-dev", HerdrKind::Herdr1))
        .unwrap_or_else(|| panic!("a pane notification has an agent"));
    assert!(resolve_target(&target, &on_one, ("pro-dev", HerdrKind::Herdr1), &snap).is_some());
    // Another host's connection does not claim it.
    assert_eq!(
        notification_agent(&target, &ConnectionKey::new("mini", HerdrKind::Herdr2)),
        None
    );
}
