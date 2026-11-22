use super::*;
use crate::hosts::test_support::{MINI, STUDIO, named_setup};
use crate::notifications::Context as Target;
use model::{observed, snapshot_content};
use sheprd_client::protocol::AgentStatus;
use target::{Destination, notification_identity, resolve_target};

#[test]
fn snapshot_agent_places_show_the_saved_name_with_or_without_a_space() {
    let setup = named_setup();
    let state = SidebarState::default();
    let mut snapshot = fixture();
    for (target, label) in [(STUDIO, "Studio Mac"), (MINI, "mini")] {
        let (_, lines) = snapshot_content(target, &snapshot, &state);
        let line = &lines[0];
        let name = setup.host_name(&line.row.identity.host);
        assert!(line.place(&name).ends_with(&format!(" · {label}")));
        assert_eq!(line.row.identity.host, target);
    }
    snapshot.workspaces.clear();
    let (_, lines) = snapshot_content(STUDIO, &snapshot, &state);
    assert_eq!(lines[0].place(&setup.host_name(STUDIO)), "Studio Mac");
}

#[test]
fn identity_survives_rename() {
    let state = SidebarState::default();
    let mut snap = fixture();
    state.observe("h", HerdrKind::Herdr1, &snap.boot_id, &observed(&snap));
    snap.agents[0].agent_status = AgentStatus::Working;
    snap.agents[0].state_change_seq += 1;
    state.observe("h", HerdrKind::Herdr1, &snap.boot_id, &observed(&snap));
    let (_, before) = snapshot_content("h", &snap, &state);
    // Rename only: the daemon reports no status change.
    snap.panes[0].label = Some("renamed".into());
    snap.agents[0].title = Some("Other title".into());
    state.observe("h", HerdrKind::Herdr1, &snap.boot_id, &observed(&snap));
    let (_, after) = snapshot_content("h", &snap, &state);
    assert_eq!(before[0].row.name, "shell");
    assert_eq!(after[0].row.name, "renamed");
    assert_eq!(after[0].row.identity, before[0].row.identity);
    assert_eq!(
        after[0].row.identity,
        AgentIdentity {
            host: "h".into(),
            kind: HerdrKind::Herdr1,
            pane: "w1:p1".into(),
        }
    );
    // The rename is not a received change, so its place in AGENTS holds.
    assert!(before[0].row.received > 0);
    assert_eq!(after[0].row.received, before[0].row.received);
}

#[test]
fn duplicate_names_show_tab_and_pane() {
    let mut a = row("h", "w1:p1", "claude", SidebarStatus::Working, 0);
    let mut b = row("h", "w1:p2", "claude", SidebarStatus::Idle, 0);
    b.tab = 2;
    let c = row("h", "w1:p3", "codex", SidebarStatus::Idle, 0);
    let space = [a.clone(), b.clone(), c.clone()];
    assert_eq!(display_name(&a, &space), "claude · tab 1 · pane w1:p1");
    assert_eq!(display_name(&b, &space), "claude · tab 2 · pane w1:p2");
    assert_eq!(display_name(&c, &space), "codex");
    // A unique name shows no numbers.
    assert_eq!(display_name(&a, std::slice::from_ref(&a)), "claude");
    // The same name on another host is not a collision in this space.
    a.identity.host = "other".into();
    assert_eq!(display_name(&a, &[a.clone(), b]), "claude");
}

#[test]
fn accessibility_name_always_includes_tab_and_pane() {
    let mut a = row("host-1", "w1:p1", "claude", SidebarStatus::NeedsYou, 0);
    a.tab = 3;
    let host = HostName::with_saved(&a.identity.host, Some("Studio Mac"));
    let spoken = accessibility_name(&a, &host);
    assert_eq!(spoken, "claude, Needs you, tab 3, pane w1:p1, Studio Mac");
    // Even with a unique shown name, the spoken name keeps the numbers.
    assert_eq!(display_name(&a, std::slice::from_ref(&a)), "claude");
    a.identity.kind = HerdrKind::Herdr2;
    assert!(accessibility_name(&a, &host).contains("tab 3, pane w1:p1"));
}

#[test]
fn notification_target_resolves_after_identity_change() {
    let mut snap = fixture();
    let target = Target {
        host: "h".into(),
        herdr: HerdrKind::Herdr1,
        generation: 1,
        boot: snap.boot_id.clone(),
        pane: Some("w1:p1".into()),
        tab: Some("w1:t1".into()),
        workspace: Some("w1".into()),
    };
    let identity =
        notification_identity(&target).unwrap_or_else(|| panic!("a pane target has an identity"));
    assert_eq!(
        identity,
        AgentIdentity {
            host: "h".into(),
            kind: HerdrKind::Herdr1,
            pane: "w1:p1".into(),
        }
    );
    // The agent is renamed and its pane moves to a new tab.
    snap.agents[0].name = Some("renamed".into());
    snap.panes[0].tab_id = "w1:t2".into();
    snap.agents[0].tab_id = "w1:t2".into();
    let connection = ("h", HerdrKind::Herdr1);
    assert_eq!(
        resolve_target(&target, &identity, connection, &snap),
        Some(Destination {
            workspace: "w1".into(),
            tab: "w1:t2".into(),
            pane: "w1:p1".into(),
        })
    );
    // Another host, another Herdr kind or another boot never resolves.
    assert_eq!(
        resolve_target(&target, &identity, ("other", HerdrKind::Herdr1), &snap),
        None
    );
    assert_eq!(
        resolve_target(&target, &identity, ("h", HerdrKind::Herdr2), &snap),
        None
    );
    let other_kind = AgentIdentity {
        kind: HerdrKind::Herdr2,
        ..identity.clone()
    };
    assert_eq!(
        resolve_target(&target, &other_kind, connection, &snap),
        None
    );
    let mut rebooted = snap.clone();
    rebooted.boot_id = "boot-2".into();
    assert_eq!(
        resolve_target(&target, &identity, connection, &rebooted),
        None
    );
    // A pane that is gone does not resolve.
    snap.panes.clear();
    assert_eq!(resolve_target(&target, &identity, connection, &snap), None);
}
