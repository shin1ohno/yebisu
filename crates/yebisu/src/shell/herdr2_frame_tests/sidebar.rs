//! L8 + L2a: Herdr 2 sessions in the sidebar column. The list comes from
//! the session's status poll; with no host or no list the column keeps only
//! the footer.
use super::*;
use crate::{
    app::Smoke,
    core::Session,
    live::Live,
    shell::{
        herdr2_frame::sidebar::{HERDR2_LIST_ID, herdr2_hosts},
        single_pane::Mailbox,
    },
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
fn herdr2_column_draws_the_list_only_with_a_session_list(cx: &mut TestAppContext) {
    let (view, cx) = frame(cx);
    show(&view, cx, None);
    assert!(
        cx.debug_bounds("shell-sidebar-list").is_some(),
        "fixture: the column (with its footer) is drawn on Herdr 2"
    );
    assert!(
        cx.debug_bounds(HERDR2_LIST_ID).is_none(),
        "no Herdr 2 list before the session has listed"
    );
    show(&view, cx, Some(listed()));
    let list = cx
        .debug_bounds(HERDR2_LIST_ID)
        .unwrap_or_else(|| panic!("the Herdr 2 list is drawn once the session listed"));
    let column = cx
        .debug_bounds("shell-sidebar-list")
        .unwrap_or_else(|| panic!("sidebar column"));
    assert!(
        column.contains(&list.origin),
        "the list sits in the sidebar column: {list:?} in {column:?}"
    );
}
