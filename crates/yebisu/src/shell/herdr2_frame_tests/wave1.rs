use super::*;
use crate::{
    actions::catalog,
    app::Smoke,
    core::{Session, SessionInfo, SessionTarget},
    shell::{
        herdr2_frame::sidebar::{CoreRow, herdr2_hosts},
        sidebar::{model::HostEntry, view::host_entries},
        sidebar_list::{ListRow, merge, sources},
        single_pane::Mailbox,
    },
};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, px, size};
use sheprd_commands::Command;
use std::time::{Duration, Instant};

fn wait_for(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    description: &str,
    condition: impl Fn(&Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        condition(this)
    }) {
        assert!(Instant::now() < deadline, "{description}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run_action(view: &Entity<Smoke>, cx: &mut VisualTestContext, command: Command) {
    view.update(cx, |this, _| {
        assert!(
            catalog(&this.terminal)
                .iter()
                .any(|a| a.info.command == command && a.available),
            "{command:?} must be available on a connected Herdr 2 terminal",
        );
        this.run_action(command)
            .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    });
}

fn open_session(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    name: &str,
    root: &std::path::Path,
) -> std::sync::mpsc::Receiver<(u32, Vec<u8>)> {
    let (backend, receipts) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("mock core: {error}"));
    view.update(cx, |this, _| {
        this.select_host("pro-dev");
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core_mode = true;
        this.terminal.live = crate::live::Live::default();
        this.terminal.core = Some(
            Session::start_named(
                SessionTarget {
                    host: "pro-dev".into(),
                    name: name.into(),
                    focus: None,
                    expected_core_id: None,
                },
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                Some(root.to_path_buf()),
                Mailbox::detached(),
            )
            .unwrap_or_else(|error| panic!("start {name}: {error}")),
        );
    });
    wait_for(view, cx, name, |this| this.terminal.live.connected);
    receipts
}

fn fixture_root(test: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("yebisu-wave1-{test}-{}", std::process::id()))
}

fn catalog_row(this: &Smoke) -> HostEntry {
    herdr2_hosts(&this.shell.sidebar, &this.terminal, &this.board.hidden)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("Herdr 2 host row"))
}

#[gpui::test]
fn herdr2_lists_every_session(cx: &mut TestAppContext) {
    let root = fixture_root("sessions");
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    open_session(&view, cx, "setup", &root);
    open_session(&view, cx, "yebisu", &root);
    run_action(&view, cx, Command::Tab);
    wait_for(&view, cx, "second tab", |this| this.core_tabs().len() == 2);
    view.update(cx, |this, _| {
        let row = catalog_row(this);
        assert_eq!(
            row.spaces
                .iter()
                .map(|space| (space.name.as_str(), space.tabs, space.current))
                .collect::<Vec<_>>(),
            [("setup", 1, false), ("yebisu", 2, true)]
        );
        this.terminal.core.take();
    });
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("clean fixtures: {error}"));
}

#[gpui::test]
fn herdr2_sessions_merge_into_the_sidebar_list(cx: &mut TestAppContext) {
    let root = fixture_root("merge");
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    open_session(&view, cx, "setup", &root);
    open_session(&view, cx, "yebisu", &root);
    view.update(cx, |this, _| {
        let readers = host_entries(&this.shell.sidebar, &this.terminal, &this.board.hidden);
        assert!(
            readers
                .iter()
                .any(|row| row.host == "pro-dev" && row.spaces.is_empty())
        );
        let mut readers = readers;
        readers.push(HostEntry::new(
            "pro-dev",
            HerdrKind::Herdr1,
            HostLink::Connected,
        ));
        let merged = merge(&sources(readers, vec![catalog_row(this)]));
        assert_eq!(
            merged
                .iter()
                .filter(|row| matches!(row, ListRow::Host(_)))
                .count(),
            1
        );
        assert_eq!(
            merged
                .iter()
                .filter_map(|row| match row {
                    ListRow::Space(space) => Some((space.entry.id.as_str(), space.generation)),
                    ListRow::Host(_) => None,
                })
                .collect::<Vec<_>>(),
            [
                ("setup", this.terminal.link.generation),
                ("yebisu", this.terminal.link.generation),
            ]
        );
        this.terminal.core.take();
    });
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("clean fixtures: {error}"));
}

#[gpui::test]
fn herdr2_tab_row_matches_herdr1(cx: &mut TestAppContext) {
    let root = fixture_root("tabs");
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let receipts = open_session(&view, cx, "setup", &root);
    let before = view.read_with(cx, |this, _| {
        let tabs = this.core_tabs();
        assert_eq!(tabs[0].label, "sh");
        tabs[0].id.clone()
    });
    run_action(&view, cx, Command::Tab);
    wait_for(&view, cx, "second tab", |this| this.core_tabs().len() == 2);
    run_action(&view, cx, Command::SplitDown);
    wait_for(&view, cx, "split down", |this| {
        this.core_tabs()
            .iter()
            .any(|tab| tab.active && tab.panes.len() == 2 && tab.vertical)
    });
    view.update(cx, |this, _| {
        let generation = this.terminal.link.generation;
        assert!(
            this.core_tab_action("pro-dev", generation.wrapping_sub(1), &before, false)
                .is_err()
        );
        this.core_tab_action("pro-dev", generation, &before, false)
            .unwrap_or_else(|error| panic!("select drawn tab: {error}"));
    });
    wait_for(&view, cx, "selected tab", |this| {
        this.core_tabs()
            .iter()
            .any(|tab| tab.id == before && tab.active)
    });
    run_action(&view, cx, Command::CloseTab);
    wait_for(&view, cx, "first tab closed", |this| {
        let tabs = this.core_tabs();
        tabs.len() == 1 && tabs[0].id != before
    });
    let (channel, bytes) = receipts
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|error| panic!("terminal.kill receipt: {error}"));
    let request: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("terminal.kill request: {error}"));
    assert_eq!(channel, 0);
    assert_eq!(request["method"], "terminal.kill");
    assert_eq!(request["params"]["terminal"], before);
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("clean fixtures: {error}"));
}

#[gpui::test]
fn herdr2_agent_row_tap_opens_its_terminal(cx: &mut TestAppContext) {
    let root = fixture_root("agent");
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    open_session(&view, cx, "yebisu", &root);
    view.update(cx, |this, cx| {
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core"));
        core.statuses_mailbox()
            .publish(vec![raw("t2", Some("working"))]);
        let row = catalog_row(this);
        let agent = &row.agents[0].row.identity;
        assert_eq!(agent.pane, "yebisu\0t2");
        assert!(
            this.open_herdr2_row(
                &row.host,
                &row.boot,
                row.generation.wrapping_sub(1),
                CoreRow::Agent(agent.pane.clone()),
                cx
            )
            .is_err()
        );
        cx.notify();
    });
    cx.run_until_parked();
    let row = cx
        .debug_bounds("herdr2-agent-0")
        .unwrap_or_else(|| panic!("Herdr 2 agent row must be drawn"));
    cx.simulate_click(row.center(), Modifiers::none());
    wait_for(&view, cx, "selected terminal t2", |this| {
        crate::board::snapshot(&this.terminal.live)
            .and_then(|snapshot| snapshot.focused_pane_id.as_deref())
            == Some("t2")
    });
    view.update(cx, |this, _| {
        let sessions: Vec<SessionInfo> = this
            .terminal
            .core
            .as_ref()
            .map(|core| core.sessions(this.terminal.live.input_epoch))
            .unwrap_or_default();
        assert_eq!(sessions[0].name, "yebisu");
        this.terminal.core.take();
    });
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("clean fixtures: {error}"));
}

#[gpui::test]
fn herdr2_external_tabs_and_agents_refresh_and_stale_saved_rows_are_unconfirmed(
    cx: &mut TestAppContext,
) {
    let root = fixture_root("observations");
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    open_session(&view, cx, "saved", &root);
    open_session(&view, cx, "current", &root);

    let first = br#"["external",["c0ffee",[{"address":"t1","state":"live","name":"agent 1","status":{"state":"working","title":"First"}},{"address":"t2","state":"live","name":"agent 2","status":{"state":"blocked","title":"Second"}}]]]
["saved",null]
"#;
    let second = br#"["external",["c0ffee",[{"address":"t2","state":"live","name":"agent 2","status":{"state":"done","title":"Second"}}]]]"#;

    view.update(cx, |this, cx| {
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("current core"));
        core.statuses_mailbox()
            .publish(vec![raw("t2", Some("working"))]);
        core.publish_discovery(Some(&root), "pro-dev", first)
            .unwrap_or_else(|error| panic!("first catalog: {error}"));

        let row = catalog_row(this);
        let external = row
            .spaces
            .iter()
            .find(|space| space.id == "external")
            .unwrap_or_else(|| panic!("external session"));
        assert_eq!(external.tabs, 2);
        assert!(external.needs_you);
        assert!(
            row.agents
                .iter()
                .any(|line| line.row.identity.pane == "external\0t1")
        );
        assert!(
            row.agents
                .iter()
                .any(|line| line.row.identity.pane == "current\0t2")
        );
        let saved = row
            .spaces
            .iter()
            .find(|space| space.id == "saved")
            .unwrap_or_else(|| panic!("saved session"));
        assert_eq!(saved.detail, "Session unconfirmed");
        let rejected = this.open_herdr2_row(
            &row.host,
            &row.boot,
            row.generation,
            CoreRow::Space("saved".into()),
            cx,
        );
        assert!(matches!(rejected, Err(error) if error.to_string().contains("unconfirmed")));

        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("current core"));
        core.publish_discovery(Some(&root), "pro-dev", second)
            .unwrap_or_else(|error| panic!("second catalog: {error}"));
        let row = catalog_row(this);
        let external = row
            .spaces
            .iter()
            .find(|space| space.id == "external")
            .unwrap_or_else(|| panic!("external session after refresh"));
        assert_eq!(external.tabs, 1);
        assert!(!external.needs_you);
        assert!(
            !row.agents
                .iter()
                .any(|line| line.row.identity.pane == "external\0t1")
        );
        assert!(row.agents.iter().any(|line| {
            line.row.identity.pane == "external\0t2" && line.row.status == SidebarStatus::Done
        }));
        this.terminal.reconnect.suspended = true;
        let rejected = this.open_herdr2_row(
            &row.host,
            &row.boot,
            row.generation,
            CoreRow::Space("external".into()),
            cx,
        );
        assert!(matches!(rejected, Err(error) if error.to_string().contains("Resume the app")));
        assert!(this.terminal.live.status.contains("Resume the app"));
        this.terminal.reconnect.suspended = false;
        let (peer, _) = crate::core_mock::recorded(false)
            .unwrap_or_else(|error| panic!("external peer: {error}"));
        this.terminal.core_test_backend = Some(peer);
        this.open_herdr2_row(
            &row.host,
            &row.boot,
            row.generation,
            CoreRow::Agent("external\0t2".into()),
            cx,
        )
        .unwrap_or_else(|error| panic!("external agent: {error}"));
        assert_eq!(this.shell.herdr2.selected_session("pro-dev"), "current");
    });
    wait_for(&view, cx, "external agent's terminal", |this| {
        this.terminal.live.connected
            && crate::board::snapshot(&this.terminal.live)
                .is_some_and(|state| state.focused_pane_id.as_deref() == Some("t2"))
    });
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert_eq!(this.shell.herdr2.selected_session("pro-dev"), "external");
        this.terminal.core.take();
    });
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("clean fixtures: {error}"));
}
