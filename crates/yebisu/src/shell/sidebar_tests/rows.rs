use super::*;
use crate::{live::Live, terminal_view::TerminalView};
use SidebarStatus::*;
use model::{HostEntry, HostLink, OfflineReason, SpaceInput, agents_list, space_entry};
use sheprd_client::ClientEvent;
use std::{collections::BTreeMap, sync::Arc};
use view::host_entries;

fn input(branch: Option<&str>) -> SpaceInput {
    SpaceInput {
        id: "w1".into(),
        name: "Space 1 · repo".into(),
        tabs: 2,
        branch: branch.map(Into::into),
        current: false,
    }
}

fn terminal(host: &str, connected: bool, status: &str) -> TerminalView {
    let mut live = Live::default();
    live.connected = true;
    live.apply(ClientEvent::Snapshot(Arc::new(fixture())));
    live.connected = connected;
    live.status = status.into();
    let mut terminal = TerminalView::default();
    terminal.reconnect.target = host.into();
    terminal.live = live;
    terminal
}

#[test]
fn offline_host_shows_no_spaces() {
    let state = SidebarState::default();
    let none = BTreeMap::new();
    let online = host_entries(&state, &terminal("h", true, ""), &none);
    assert_eq!(online[0].link, HostLink::Connected);
    assert_eq!(online[0].spaces.len(), 1, "positive control");
    assert_eq!(online[0].detail(), "SSH · Herdr 1");
    // The same reader, now disconnected, still holds the last snapshot.
    let reason = "host is untrusted or its key has changed; authentication refused";
    let offline = host_entries(&state, &terminal("h", false, reason), &none);
    assert_eq!(
        offline[0].link,
        HostLink::Offline(OfflineReason::HostKeyMismatch)
    );
    assert!(offline[0].spaces.is_empty());
    assert!(offline[0].agents.is_empty());
    assert_eq!(offline[0].detail(), "Host key mismatch");
    let connecting = host_entries(&state, &terminal("h", false, "Connecting…"), &none);
    assert!(connecting[0].spaces.is_empty());
    assert_eq!(connecting[0].detail(), "Connecting…");
    for (text, label) in [
        (
            "SSH operation failed: authentication failed",
            "Sign-in failed",
        ),
        ("herdr socket: connection refused", "Herdr not running"),
        ("Version mismatch: Protocol", "Version mismatch"),
        ("closed", "Not connected"),
    ] {
        let entry = host_entries(&state, &terminal("h", false, text), &none);
        assert_eq!(entry[0].detail(), label, "{text}");
        assert!(entry[0].spaces.is_empty(), "{text}");
    }
    // The model refuses spaces for a host that is not connected.
    let space = space_entry(HerdrKind::Herdr1, input(None), &[]);
    let kept = HostEntry::new("h", HerdrKind::Herdr1, HostLink::Connecting)
        .with_content(vec![space], vec![]);
    assert!(kept.spaces.is_empty());
}

#[test]
fn branch_only_from_herdr1() {
    let agent = [row("h", "p", "claude", Working, 0)];
    let herdr1 = space_entry(HerdrKind::Herdr1, input(Some("main")), &agent);
    assert_eq!(herdr1.detail, "claude · Working · main");
    let herdr2 = space_entry(HerdrKind::Herdr2, input(Some("main")), &agent);
    assert_eq!(herdr2.detail, "claude · Working");
    // A cwd shown as `~` is not a branch.
    let home = space_entry(HerdrKind::Herdr1, input(Some("~/src")), &agent);
    assert_eq!(home.detail, "claude · Working");
    let none = space_entry(HerdrKind::Herdr1, input(None), &agent);
    assert_eq!(none.detail, "claude · Working");
}

#[test]
fn status_less_terminal_is_not_an_agent() {
    let shell = row("h", "p1", "zsh", NoStatus, 9);
    let space = space_entry(HerdrKind::Herdr2, input(None), std::slice::from_ref(&shell));
    assert_eq!(space.detail, "No agent");
    assert_eq!(space.dot, NoStatus);
    let agent = row("h", "p2", "claude", Idle, 0);
    let mixed = space_entry(
        HerdrKind::Herdr2,
        input(None),
        &[shell.clone(), agent.clone()],
    );
    assert_eq!(mixed.detail, "claude · Idle");
    let mut host = HostEntry::new("h", HerdrKind::Herdr2, HostLink::Connected);
    host.agents = [shell, agent]
        .into_iter()
        .map(|row| model::AgentLine {
            shown: row.name.clone(),
            place: "Space · h".into(),
            row,
        })
        .collect();
    let listed = agents_list(&[host]);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].row.name, "claude");
}

#[test]
fn space_row_shows_single_error_tag() {
    let rows = [
        row("h", "p1", "a", Error, 0),
        row("h", "p2", "b", Error, 0),
        row("h", "p3", "c", NeedsYou, 0),
    ];
    let space = space_entry(HerdrKind::Herdr2, input(None), &rows);
    assert_eq!(space.tags(), ["Needs you", "Error"]);
    assert_eq!(space.dot, NeedsYou);
    let only_errors = space_entry(HerdrKind::Herdr2, input(None), &rows[..2]);
    assert_eq!(only_errors.tags(), ["Error"]);
    assert_eq!(only_errors.dot, Error);
    let calm = space_entry(
        HerdrKind::Herdr2,
        input(None),
        &[row("h", "p", "d", Done, 0)],
    );
    assert!(calm.tags().is_empty());
}

#[test]
fn board_density_toggle_removed() {
    let board = include_str!("../../board.rs");
    let sidebar = include_str!("../sidebar/view.rs");
    for source in [board, sidebar] {
        for gone in [
            "\"board-density\"",
            "\"Board: Dense\"",
            "\"Board: Roomy\"",
            ".roomy",
        ] {
            assert!(!source.contains(gone), "{gone} is back");
        }
    }
    assert!(board.contains("view::HOSTS_HEADING"));
    assert_eq!(view::HOSTS_HEADING, "HOSTS");
    assert_eq!(crate::board::BOARD_HEIGHT, 180.);
}

#[test]
fn row_height_follows_text_scale() {
    assert_eq!(row_height(AGENT_ROW_HEIGHT, 1.0), 44.);
    assert_eq!(row_height(SPACE_ROW_HEIGHT, 1.0), 56.);
    assert_eq!(row_height(HOST_ROW_HEIGHT, 1.0), 64.);
    assert_eq!(row_height(AGENT_ROW_HEIGHT, 1.5), 66.);
    assert_eq!(row_height(HOST_ROW_HEIGHT, 2.0), 128.);
    // Never shorter than the base touch height.
    assert_eq!(row_height(AGENT_ROW_HEIGHT, 0.8), 44.);
    assert_eq!(row_height(AGENT_ROW_HEIGHT, f32::NAN), 44.);
}
