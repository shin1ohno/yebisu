use super::*;
use crate::{app::Smoke, hosts::HostName, live::Live, settings::sidebar_density::SidebarDensity};
use gpui::{AppContext as _, Modifiers, TestAppContext, VisualTestContext, point, px, size};
use model::{AgentLine, HostEntry, HostLink, OfflineReason};
use sheprd_client::ClientEvent;
use std::sync::Arc;
use view::{Badge, Dot, SidebarTap};

fn click(cx: &mut VisualTestContext, id: &'static str) {
    let bounds = cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    cx.simulate_click(
        point(
            bounds.origin.x + bounds.size.width / 2.,
            bounds.origin.y + bounds.size.height / 2.,
        ),
        Modifiers::none(),
    );
    cx.run_until_parked();
}

#[gpui::test]
fn sidebar_header_has_collapse_title_search_and_new_space(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |app, cx| {
        app.terminal.open = true;
        cx.notify();
    });
    cx.run_until_parked();
    let controls = [
        "shell-sidebar-toggle",
        "sidebar-title",
        "sidebar-filter-unavailable",
        "shell-footer-new-space",
    ];
    let bounds = controls.map(|id| {
        cx.debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"))
    });
    assert!(
        bounds
            .windows(2)
            .all(|pair| pair[0].origin.x < pair[1].origin.x)
    );
    for control in [bounds[0], bounds[2], bounds[3]] {
        assert_eq!(control.size, size(px(44.), px(44.)));
        assert_eq!(control.origin.y, bounds[0].origin.y);
    }
    assert!(
        cx.debug_bounds("shell-footer-settings")
            .is_some_and(|settings| { settings.origin.y > bounds[3].origin.y })
    );
}

#[gpui::test]
fn host_row_chevron_collapses_its_spaces(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let snapshot = fixture();
    assert_eq!(snapshot.workspaces[0].workspace_id, "w1");
    let space = "sidebar-space-pro-dev-w1";
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |app, cx| {
        app.terminal.reconnect.target = "pro-dev".into();
        app.terminal.open = true;
        app.terminal.live = Live::default();
        app.terminal.live.connected = true;
        app.terminal
            .live
            .apply(ClientEvent::Snapshot(Arc::new(snapshot)));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(space).is_some(),
        "connected fixture has a space"
    );
    click(cx, "sidebar-chevron-pro-dev");
    assert!(cx.debug_bounds(space).is_none());
    click(cx, "sidebar-chevron-pro-dev");
    assert!(cx.debug_bounds(space).is_some());
}

#[test]
fn host_row_second_line_names_ssh_herdr_and_state() {
    for (kind, link, expected) in [
        (
            HerdrKind::Herdr1,
            HostLink::Connected,
            "SSH · Herdr 1 · Connected",
        ),
        (
            HerdrKind::Herdr2,
            HostLink::Connecting,
            "SSH · Herdr 2 · Connecting…",
        ),
        (
            HerdrKind::Herdr1,
            HostLink::Offline(OfflineReason::SignInFailed),
            "SSH · Herdr 1 · Sign-in failed",
        ),
    ] {
        assert_eq!(
            HostEntry::new("h", kind, link).connection_detail(),
            expected
        );
    }
}

#[test]
fn host_rows_show_the_display_name() {
    let destination = "user@pro-dev.home.local";
    let host = HostEntry::new(destination, HerdrKind::Herdr1, HostLink::Connected);
    assert_eq!(host.display_name(), "pro-dev");
    assert_eq!(host.host, HostName::new(destination).destination());
}

#[test]
fn connecting_host_dot_is_a_yellow_ring() {
    assert_eq!(
        view::host_dot(HostLink::Connecting),
        Dot {
            color: 0xebcb8b,
            stroke: 2.
        }
    );
    assert_eq!(view::host_dot(HostLink::Connected).stroke, 0.);
}

#[test]
fn space_row_shows_label_and_singular_tab_count() {
    let mut snapshot = fixture();
    snapshot.workspaces[0].label = "session-search".into();
    let state = SidebarState::default();
    let (spaces, _) = model::snapshot_content("pro-dev", &snapshot, &state);
    assert_eq!(spaces[0].name, "session-search");
    assert_eq!(spaces[0].tab_count(), "1 tab");
    snapshot.workspaces[0].label.clear();
    snapshot.tabs.clear();
    let (spaces, _) = model::snapshot_content("pro-dev", &snapshot, &state);
    assert_eq!(
        spaces[0].name,
        format!("Space {}", snapshot.workspaces[0].number)
    );
    assert_eq!(spaces[0].tab_count(), "0 tabs");
}

#[test]
fn needs_you_badge_and_error_tag_follow_the_canvas() {
    assert_eq!(
        Badge::for_tag("Needs you"),
        Badge {
            ink: 0x101419,
            fill: Some(0xebcb8b),
            outline: false
        }
    );
    assert_eq!(
        Badge::for_tag("Error"),
        Badge {
            ink: 0xbf616a,
            fill: None,
            outline: true
        }
    );
}

#[test]
fn agents_list_puts_pinned_after_needs_you_and_error() {
    use SidebarStatus::*;
    let mut host = HostEntry::new("h", HerdrKind::Herdr1, HostLink::Connected);
    host.boot = "b".into();
    host.agents = [
        ("working", Working),
        ("error", Error),
        ("second pin", Idle),
        ("done", Done),
        ("first pin", Done),
        ("waiting", NeedsYou),
        ("idle", Idle),
    ]
    .into_iter()
    .map(|(name, status)| AgentLine {
        row: row("h", name, name, status, 0),
        shown: name.into(),
        place: "space · h".into(),
    })
    .collect();
    let lines = model::agents_list_with_pins(&[host], &|id, boot| {
        assert_eq!(boot, "b");
        ["second pin", "first pin", "waiting", "error"]
            .iter()
            .position(|pane| *pane == id.pane)
    });
    assert_eq!(
        lines
            .iter()
            .map(|line| line.shown.as_str())
            .collect::<Vec<_>>(),
        [
            "waiting",
            "error",
            "second pin",
            "first pin",
            "working",
            "done",
            "idle"
        ]
    );
}

#[test]
fn agent_row_shows_task_title_and_pane_name() {
    let mut snapshot = fixture();
    snapshot.agents[0].title = Some("Review\n release".into());
    snapshot.agents[0].name = Some("Registered name".into());
    let pane = snapshot.agents[0].pane_id.clone();
    let pane_id = snapshot
        .panes
        .iter()
        .position(|row| row.pane_id == pane)
        .unwrap_or_else(|| panic!("missing fixture pane"));
    snapshot.panes[pane_id].label = None;
    let state = SidebarState::default();
    let title = |snapshot: &sheprd_client::protocol::ClientShellSnapshot| {
        model::snapshot_content("h", snapshot, &state).1[0]
            .shown
            .clone()
    };
    assert_eq!(title(&snapshot), "Review release");
    snapshot.panes[pane_id].label = Some("Renamed pane".into());
    assert_eq!(title(&snapshot), "Renamed pane");
    snapshot.panes[pane_id].label = None;
    snapshot.agents[0].title = Some("Claude Code".into());
    snapshot.agents[0].name = Some("agent".into());
    snapshot.agents[0].display_agent = None;
    assert!(title(&snapshot).starts_with(&format!("{pane} · ")));
    snapshot.agents[0].title = Some("a".repeat(200));
    assert_eq!(title(&snapshot).chars().count(), 160);
}

#[test]
fn the_space_reason_uses_the_leading_blocked_agent_label() {
    let mut snapshot = fixture();
    snapshot.agents[0].agent_status = sheprd_client::protocol::AgentStatus::Blocked;
    snapshot.agents[0].state_labels = vec![("blocked".into(), "Permission: run tests".into())];
    let (spaces, _) = model::snapshot_content("h", &snapshot, &SidebarState::default());
    assert!(spaces[0].detail.contains(" · Permission: run tests"));
}

#[gpui::test]
fn space_tap_after_reconnect_uses_the_drawn_connection(cx: &mut TestAppContext) {
    let app = cx.new(Smoke::new);
    app.update(cx, |app, _| {
        app.select_host("h");
        app.terminal.live.connected = true;
        app.terminal
            .live
            .apply(ClientEvent::Snapshot(Arc::new(fixture())));
        let hosts = view::host_entries(&app.shell.sidebar, &app.terminal, &app.board.hidden);
        let drawn = view::space_tap(&hosts[0], &hosts[0].spaces[0]);
        let SidebarTap::Space { generation, .. } = &drawn else {
            panic!("expected a space tap");
        };
        assert_eq!(*generation, app.terminal.link.generation);

        app.terminal.link.reset();
        let mut inbox = app
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|err| panic!("{err}"));
        inbox.connected = true;
        inbox.apply(ClientEvent::Snapshot(Arc::new(fixture())));
        drop(inbox);
        let Err(error) = app.sidebar_tap(drawn) else {
            panic!("the old row was accepted");
        };
        assert!(error.to_string().contains("Stale board row"), "{error}");
    });
}

#[test]
fn sidebar_density_changes_row_heights() {
    let mut state = SidebarState::default();
    let bases = [HOST_ROW_HEIGHT, SPACE_ROW_HEIGHT, AGENT_ROW_HEIGHT];
    assert_eq!(bases.map(|h| state.sidebar_row_height(h)), [64., 56., 44.]);
    state.density = SidebarDensity::Roomy;
    assert_eq!(bases.map(|h| state.sidebar_row_height(h)), [72., 64., 52.]);
}
