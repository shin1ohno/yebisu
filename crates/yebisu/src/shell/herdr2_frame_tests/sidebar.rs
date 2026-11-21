//! Herdr 2 sessions share the sidebar column and row layout with Herdr 1.
use super::*;
use crate::{
    app::Smoke,
    core::Session,
    live::Live,
    shell::{herdr2_frame::sidebar::herdr2_hosts, single_pane::Mailbox},
};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// A session that never connects; the test writes its statuses.
fn listing_session(statuses: Option<Vec<CoreStatus>>) -> Session {
    let session = Session::start(
        "sidebar-fixture".into(),
        1,
        Arc::new(Mutex::new(Live::default())),
        || Err(crate::Error::Ssh("fixture: never connects".into())),
        None,
        Mailbox::detached(),
    )
    .unwrap_or_else(|e| panic!("session: {e}"));
    if let Some(statuses) = statuses {
        session.statuses_mailbox().publish(statuses);
    }
    session
}

fn core_terminal(host: &str, statuses: Option<Vec<CoreStatus>>) -> TerminalView {
    let mut terminal = TerminalView {
        core_mode: true,
        core: Some(listing_session(statuses)),
        ..TerminalView::default()
    };
    terminal.reconnect.target = host.into();
    terminal.live.connected = true;
    terminal
}

fn listed() -> Vec<CoreStatus> {
    vec![raw("t1", None), raw("t2", Some("error"))]
}

#[test]
fn herdr2_hosts_come_only_from_a_session_list() {
    let state = SidebarState::default();
    let hidden = BTreeMap::new();
    let hosts = herdr2_hosts(&state, &core_terminal("pro-dev", Some(listed())), &hidden);
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].kind, HerdrKind::Herdr2);
    assert!(hosts[0].selected);
    assert_eq!(hosts[0].spaces.len(), 1, "one session row");
    let agents = hosts[0]
        .agents
        .iter()
        .map(|l| (l.row.identity.pane.as_str(), l.row.status))
        .collect::<Vec<_>>();
    assert_eq!(agents, [("t2", SidebarStatus::Error)]);

    // No list yet, no session, no host, or Herdr 1: no Herdr 2 row.
    assert!(herdr2_hosts(&state, &core_terminal("pro-dev", None), &hidden).is_empty());
    let mut no_session = core_terminal("pro-dev", Some(listed()));
    no_session.core = None;
    assert!(herdr2_hosts(&state, &no_session, &hidden).is_empty());
    assert!(herdr2_hosts(&state, &core_terminal("", Some(listed())), &hidden).is_empty());
    let mut herdr1 = core_terminal("pro-dev", Some(listed()));
    herdr1.core_mode = false;
    assert!(herdr2_hosts(&state, &herdr1, &hidden).is_empty());

    // Not connected: the host row stays, without its session.
    let mut offline = core_terminal("pro-dev", Some(listed()));
    offline.live.connected = false;
    let offline = herdr2_hosts(&state, &offline, &hidden);
    assert!(offline[0].spaces.is_empty() && offline[0].agents.is_empty());
}

#[test]
fn a_hidden_herdr2_host_keeps_its_session_row() {
    let state = SidebarState::default();
    let mut hidden = BTreeMap::new();
    hidden.insert("mini".to_owned(), core_terminal("mini", Some(listed())));
    let hosts = herdr2_hosts(&state, &core_terminal("pro-dev", Some(listed())), &hidden);
    let rows = hosts
        .iter()
        .map(|h| (h.host.as_str(), h.selected, h.spaces.len()))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows.contains(&("mini", false, 1)) && rows.contains(&("pro-dev", true, 1)));
}

fn frame(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    (view, cx)
}

/// Shows `terminal` in the frame (the frame draws only an open terminal).
fn show(view: &Entity<Smoke>, cx: &mut VisualTestContext, statuses: Option<Vec<CoreStatus>>) {
    view.update(cx, |this, cx| {
        this.terminal = core_terminal("pro-dev", statuses);
        this.terminal.open = true;
        cx.notify();
    });
    cx.run_until_parked();
}

#[gpui::test]
fn herdr2_draws_the_shared_sidebar_before_and_after_listing(cx: &mut TestAppContext) {
    let (view, cx) = frame(cx);
    show(&view, cx, None);
    assert!(cx.debug_bounds("sidebar-host-pro-dev").is_some());
    assert!(
        cx.debug_bounds("sidebar-core-space-pro-dev-default")
            .is_none()
    );
    show(&view, cx, Some(listed()));
    let list = cx
        .debug_bounds("sidebar-list")
        .unwrap_or_else(|| panic!("shared sidebar list"));
    let column = cx
        .debug_bounds("shell-sidebar-list")
        .unwrap_or_else(|| panic!("sidebar column"));
    assert!(column.contains(&list.origin));
    assert!(
        cx.debug_bounds("sidebar-core-space-pro-dev-default")
            .is_some()
    );
    assert!(cx.debug_bounds("sidebar-core-agent-pro-dev-t2").is_some());
}

#[gpui::test]
fn sidebar_draws_one_list_for_herdr1_and_herdr2(cx: &mut TestAppContext) {
    let (view, cx) = frame(cx);
    let client =
        crate::mock_peer::connect_board().unwrap_or_else(|error| panic!("Herdr 1: {error}"));
    view.update(cx, |this, _| {
        this.select_host("pro-dev");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let link = loop {
        cx.run_until_parked();
        if let Some(ready) = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            let snapshot = crate::board::snapshot(&this.terminal.live)?;
            let workspace = snapshot.workspaces.first()?;
            assert_eq!(workspace.workspace_id, "w1");
            Some(this.terminal.link.inbox.clone())
        }) {
            break ready;
        }
        assert!(
            std::time::Instant::now() < until,
            "Herdr 1 fixture did not publish"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    };

    view.update(cx, |this, cx| {
        this.select_host_kind("pro-dev", HerdrKind::Herdr2);
        this.terminal = core_terminal("pro-dev", Some(listed()));
        this.terminal.open = true;
        assert!(Arc::ptr_eq(&link, &this.board.hidden["pro-dev"].link.inbox));
        cx.notify();
    });
    cx.run_until_parked();

    let h1 = "sidebar-space-pro-dev-w1";
    let h2 = "sidebar-core-space-pro-dev-default";
    let host = cx
        .debug_bounds("sidebar-host-pro-dev")
        .unwrap_or_else(|| panic!("one host row"));
    let h1 = cx
        .debug_bounds(h1)
        .unwrap_or_else(|| panic!("listed Herdr 1 space"));
    let h2 = cx
        .debug_bounds(h2)
        .unwrap_or_else(|| panic!("listed Herdr 2 session"));
    assert!(host.origin.y < h1.origin.y && h1.origin.y < h2.origin.y);
    assert!(cx.debug_bounds("sidebar-core-agent-pro-dev-t2").is_some());

    cx.simulate_click(h1.center(), gpui::Modifiers::none());
    view.read_with(cx, |this, _| {
        assert!(!this.terminal.core_mode);
        assert!(Arc::ptr_eq(&link, &this.terminal.link.inbox));
        assert!(this.board.hidden_core["pro-dev"].core.is_some());
    });
    cx.run_until_parked();
    cx.simulate_click(h2.center(), gpui::Modifiers::none());
    view.read_with(cx, |this, _| {
        assert!(this.terminal.core_mode);
        assert!(Arc::ptr_eq(&link, &this.board.hidden["pro-dev"].link.inbox));
    });
}
