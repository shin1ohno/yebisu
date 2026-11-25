//! Sidebar rows of a host whose `default` session reads `herdr-app` (#144
//! D2, AC1, AC2, AC11): one row per document space in its order, the
//! iPad-only `default` row, every other session "iPad only", a tap on a
//! document space row draws that space, and New tab there writes it (D4).
use super::*;
use crate::{
    app::Smoke,
    core::{DocCatalog, DocRows, DocSpace, Session, SessionInfo, TabInfo, herdr_doc_sync},
    core_mock::Host,
    shell::{
        herdr2_frame::sidebar::{CoreRow, herdr2_hosts},
        single_pane::Mailbox,
    },
};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::time::{Duration, Instant};

const A: &str = "sp000000000000000000000000000000aa";
const B: &str = "sp000000000000000000000000000000bb";

fn tab(id: &str, panes: &[&str]) -> TabInfo {
    TabInfo {
        id: id.into(),
        label: id.into(),
        panes: panes.iter().map(|p| (*p).to_owned()).collect(),
        active: false,
        focused_pane: 0,
        vertical: false,
        auto_name: false,
    }
}

fn space(id: &str, name: &str, tabs: Vec<TabInfo>) -> DocSpace {
    DocSpace {
        id: id.into(),
        name: name.into(),
        auto_name: false,
        folder: String::new(),
        tabs,
    }
}

fn session(name: &str, tabs: Vec<TabInfo>, doc: Option<DocCatalog>) -> SessionInfo {
    SessionInfo {
        name: name.into(),
        tabs,
        confirmed: true,
        core_id: None,
        statuses: Vec::new(),
        created: 0,
        doc,
    }
}

fn readable(spaces: Vec<DocSpace>, selected: Option<&str>, local_tabs: usize) -> DocCatalog {
    DocCatalog {
        state: DocRows::Readable(spaces),
        selected: selected.map(str::to_owned),
        local_tabs,
        version: 1,
    }
}

fn rows_of(catalog: &[SessionInfo], statuses: &[CoreStatus]) -> Vec<(String, String, usize, bool)> {
    let state = SidebarState::default();
    rows::catalog_content("pro-dev", 1, "default", catalog, statuses, &state)
        .0
        .into_iter()
        .map(|s| (s.id, s.detail, s.tabs, s.current))
        .collect()
}

fn row(id: &str, detail: &str, tabs: usize, current: bool) -> (String, String, usize, bool) {
    (id.into(), detail.into(), tabs, current)
}

#[test]
fn rows_follow_document_order() {
    let doc = readable(
        vec![
            space(B, "Build", vec![tab("tb1", &["t1"]), tab("tb2", &["t2"])]),
            space(A, "alpha", vec![tab("tb3", &["t3"])]),
        ],
        Some(A),
        0,
    );
    let catalog = [session("default", vec![tab("tb3", &["t3"])], Some(doc))];
    assert_eq!(
        rows_of(&catalog, &[]),
        [
            row(&format!("default\0{B}"), "No agent", 2, false),
            row(&format!("default\0{A}"), "No agent", 1, true),
        ]
    );
    let state = SidebarState::default();
    let names = rows::catalog_content("pro-dev", 1, "default", &catalog, &[], &state)
        .0
        .into_iter()
        .map(|s| s.name)
        .collect::<Vec<_>>();
    assert_eq!(names, ["Build", "alpha"]);
}

#[test]
fn null_document_has_no_doc_rows() {
    // An empty document: only the iPad-only tabs, while there are any.
    let with_tabs = [session(
        "default",
        vec![tab("t1", &["t1"])],
        Some(readable(Vec::new(), None, 1)),
    )];
    assert_eq!(
        rows_of(&with_tabs, &[]),
        [row("default", "iPad only", 1, true)]
    );
    let none = [session(
        "default",
        Vec::new(),
        Some(readable(Vec::new(), None, 0)),
    )];
    assert!(rows_of(&none, &[]).is_empty());
}

#[test]
fn ipad_only_row_follows_the_document_spaces() {
    let doc = readable(vec![space(A, "alpha", vec![tab("tb1", &["t1"])])], None, 2);
    let catalog = [session(
        "default",
        vec![tab("t8", &["t8"]), tab("t9", &["t9"])],
        Some(doc),
    )];
    assert_eq!(
        rows_of(&catalog, &[]),
        [
            row(&format!("default\0{A}"), "No agent", 1, false),
            row("default", "iPad only", 2, true),
        ]
    );
}

#[test]
fn unreadable_shows_one_row_with_reason() {
    let doc = DocCatalog {
        state: DocRows::Unreadable,
        selected: None,
        local_tabs: 1,
        version: 1,
    };
    let catalog = [session("default", vec![tab("t1", &["t1"])], Some(doc))];
    assert_eq!(
        rows_of(&catalog, &[]),
        [row("default", "Can't read the herdr-app layout", 1, true)]
    );
}

#[test]
fn other_sessions_are_ipad_only() {
    let doc = readable(
        vec![space(A, "alpha", vec![tab("tb1", &["t1"])])],
        Some(A),
        0,
    );
    let mut unconfirmed = session("saved", vec![tab("t1", &["t1"])], None);
    unconfirmed.confirmed = false;
    let catalog = [
        session("default", Vec::new(), Some(doc)),
        session("space-1700000000000", vec![tab("t1", &["t1"])], None),
        unconfirmed,
    ];
    let rows = rows_of(&catalog, &[]);
    assert_eq!(rows[1], row("space-1700000000000", "iPad only", 1, false));
    // "Session unconfirmed" still wins (the row cannot be opened).
    assert_eq!(rows[2], row("saved", "Session unconfirmed", 1, false));
}

#[test]
fn other_sessions_keep_their_rows_without_a_document() {
    let catalog = [
        session("default", vec![tab("t1", &["t1"])], None),
        session("setup", vec![tab("t1", &["t1"])], None),
    ];
    assert_eq!(
        rows_of(&catalog, &[]),
        [
            row("default", "No agent", 1, true),
            row("setup", "No agent", 1, false)
        ]
    );
}

#[test]
fn ids_never_collide() {
    // A document space row is NUL-namespaced; a session name cannot be.
    let doc = readable(
        vec![space("setup", "setup", vec![tab("tb1", &["t1"])])],
        None,
        1,
    );
    let catalog = [
        session("default", vec![tab("t9", &["t9"])], Some(doc)),
        session("setup", vec![tab("t1", &["t1"])], None),
    ];
    let ids = rows_of(&catalog, &[])
        .into_iter()
        .map(|(id, ..)| id)
        .collect::<Vec<_>>();
    assert_eq!(ids, ["default\0setup", "default", "setup"]);
    assert_eq!(
        rows::split_space_identity("default\0setup"),
        ("default", Some("setup"))
    );
    assert_eq!(rows::split_space_identity("setup"), ("setup", None));
}

#[test]
fn agents_follow_the_document_tabs() {
    let doc = readable(
        vec![space(
            A,
            "alpha",
            vec![tab("tb1", &["t1"]), tab("tb2", &["t5", "t2"])],
        )],
        Some(A),
        1,
    );
    let catalog = [session("default", vec![tab("tb1", &["t1"])], Some(doc))];
    let statuses = [raw("t2", Some("working")), raw("t9", Some("blocked"))];
    let state = SidebarState::default();
    let (spaces, lines) =
        rows::catalog_content("pro-dev", 1, "default", &catalog, &statuses, &state);
    let place = lines
        .iter()
        .map(|line| {
            (
                line.row.identity.pane.as_str(),
                line.space.as_deref(),
                line.row.tab,
            )
        })
        .collect::<Vec<_>>();
    assert!(place.contains(&("t2", Some("alpha"), 2)), "{place:?}");
    assert!(place.contains(&("t9", Some("default"), 1)), "{place:?}");
    let alpha = spaces
        .iter()
        .find(|s| s.id == format!("default\0{A}"))
        .unwrap_or_else(|| panic!("alpha row"));
    assert!(alpha.detail.starts_with("agent t2"), "{}", alpha.detail);
    let local = spaces
        .iter()
        .find(|s| s.id == "default")
        .unwrap_or_else(|| panic!("default row"));
    assert_eq!(local.detail, "iPad only");
    assert!(local.needs_you);
}

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

fn tab_labels(this: &Smoke) -> Vec<String> {
    this.core_tabs().into_iter().map(|tab| tab.label).collect()
}

#[gpui::test]
fn document_space_rows_open_their_space(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("yebisu-doc-rows-{}", std::process::id()));
    let host = Host::default();
    host.doc().set_silent(
        serde_json::from_str(include_str!("../../../fixtures/herdr-app/two-spaces.json"))
            .unwrap_or_else(|e| panic!("fixture: {e}")),
    );
    let (backend, _) = host.recorded(false).unwrap_or_else(|e| panic!("mock: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.select_host("pro-dev");
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core_mode = true;
        this.terminal.live = crate::live::Live::default();
        this.terminal.core = Some(
            Session::start_doc(
                "pro-dev".into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                Some(root.clone()),
                Mailbox::detached(),
                herdr_doc_sync::DocPlan {
                    source: herdr_doc_sync::DocSource::Core,
                    period: Duration::from_millis(200),
                },
            )
            .unwrap_or_else(|e| panic!("start: {e}")),
        );
    });
    wait_for(&view, cx, "the first document space", |this| {
        tab_labels(this) == ["1", "logs"]
    });
    view.update(cx, |this, cx| {
        let row = herdr2_hosts(&this.shell.sidebar, &this.terminal, &this.board.hidden)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("host row"));
        let names = row
            .spaces
            .iter()
            .map(|s| (s.name.as_str(), s.tabs, s.current))
            .collect::<Vec<_>>();
        assert_eq!(names, [("Build", 2, true), ("beta", 1, false)]);
        let second = row.spaces[1].id.clone();
        this.open_herdr2_row(
            &row.host,
            &row.boot,
            row.generation,
            CoreRow::Space(second),
            cx,
        )
        .unwrap_or_else(|e| panic!("open: {e}"));
    });
    // The second space's only terminal (`t30`) is not live: no tab draws,
    // and its row is the current one.
    wait_for(&view, cx, "the second document space", |this| {
        let current = herdr2_hosts(&this.shell.sidebar, &this.terminal, &this.board.hidden)
            .first()
            .and_then(|row| row.spaces.get(1).map(|space| space.current));
        tab_labels(this).is_empty() && current == Some(true) && this.terminal.live.connected
    });
    // #144 D4: New tab in a document space writes the document, and the
    // new tab is drawn (and chosen) once the read after the write has it.
    view.update(cx, |this, _| {
        this.try_core_command(crate::core::Command::NewTab)
            .unwrap_or_else(|e| panic!("send: {e}"));
    });
    // The space's first tab (`t30`) is not live, so the new one, second in
    // the document, is the only tab drawn.
    wait_for(&view, cx, "the new tab", |this| {
        tab_labels(this) == ["2"] && this.core_tabs().first().is_some_and(|tab| tab.active)
    });
    assert_eq!(host.doc().sets().len(), 1, "one doc.set");
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
    let _ = std::fs::remove_dir_all(root);
}

#[gpui::test]
fn a_document_change_elsewhere_keeps_the_input_epoch(cx: &mut TestAppContext) {
    // Input is aimed at the publication's epoch (terminal_input::destination):
    // a space added on the Mac must update the rows without making it stale.
    let root = std::env::temp_dir().join(format!("yebisu-doc-epoch-{}", std::process::id()));
    let host = Host::default();
    let mut doc: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/herdr-app/two-spaces.json"))
            .unwrap_or_else(|e| panic!("fixture: {e}"));
    host.doc().set_silent(doc.clone());
    let (backend, _) = host.recorded(false).unwrap_or_else(|e| panic!("mock: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.select_host("pro-dev");
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core_mode = true;
        this.terminal.live = crate::live::Live::default();
        this.terminal.core = Some(
            Session::start_doc(
                "pro-dev".into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                Some(root.clone()),
                Mailbox::detached(),
                herdr_doc_sync::DocPlan {
                    source: herdr_doc_sync::DocSource::Core,
                    period: Duration::from_millis(200),
                },
            )
            .unwrap_or_else(|e| panic!("start: {e}")),
        );
    });
    let spaces = |this: &Smoke| {
        herdr2_hosts(&this.shell.sidebar, &this.terminal, &this.board.hidden)
            .first()
            .map_or(0, |row| row.spaces.len())
    };
    wait_for(&view, cx, "the document is drawn", |this| {
        tab_labels(this) == ["1", "logs"] && spaces(this) == 2 && this.terminal.live.connected
    });
    let epoch = view.update(cx, |this, _| this.terminal.live.input_epoch);
    if let Some(spaces) = doc["spaces"].as_array_mut() {
        spaces.push(serde_json::json!({"id": "spcc", "name": "Third", "tabs": []}));
    }
    host.doc().set_external(doc);
    wait_for(&view, cx, "the third space's row", |this| spaces(this) == 3);
    view.update(cx, |this, _| {
        assert_eq!(
            this.terminal.live.input_epoch, epoch,
            "the drawn panes did not change"
        );
        assert_eq!(tab_labels(this), ["1", "logs"]);
        this.terminal.core.take();
    });
    let _ = std::fs::remove_dir_all(root);
}

/// AC11: while `default` reads the document, a session other than `default`
/// keeps its own "iPad only" row next to the document space rows, and
/// pressing that row still opens the session.
#[gpui::test]
fn other_session_row_still_opens(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("yebisu-doc-other-{}", std::process::id()));
    let host = Host::default();
    host.doc().set_silent(
        serde_json::from_str(include_str!("../../../fixtures/herdr-app/two-spaces.json"))
            .unwrap_or_else(|e| panic!("fixture: {e}")),
    );
    let (backend, _) = host.recorded(false).unwrap_or_else(|e| panic!("mock: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.select_host("pro-dev");
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core_mode = true;
        this.terminal.live = crate::live::Live::default();
        this.terminal.core = Some(
            Session::start_doc(
                "pro-dev".into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                Some(root.clone()),
                Mailbox::detached(),
                herdr_doc_sync::DocPlan {
                    source: herdr_doc_sync::DocSource::Core,
                    period: Duration::from_millis(200),
                },
            )
            .unwrap_or_else(|e| panic!("start: {e}")),
        );
    });
    wait_for(&view, cx, "the first document space", |this| {
        tab_labels(this) == ["1", "logs"] && this.terminal.live.connected
    });
    view.update(cx, |this, cx| {
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("default core"));
        core.publish_discovery(
            Some(&root),
            "pro-dev",
            br#"["setup",["c0ffee",[{"address":"t1","state":"live","name":"shell","status":{"state":"idle","title":"Shell"}}]]]"#,
        )
        .unwrap_or_else(|e| panic!("catalog: {e}"));
        let row = herdr2_hosts(&this.shell.sidebar, &this.terminal, &this.board.hidden)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("host row"));
        let rows = row
            .spaces
            .iter()
            .map(|s| (s.id.as_str(), s.name.as_str(), s.detail.as_str()))
            .collect::<Vec<_>>();
        let documented = rows
            .iter()
            .filter(|(id, _, _)| id.starts_with("default\0"))
            .map(|(_, name, _)| *name)
            .collect::<Vec<_>>();
        assert_eq!(documented, ["Build", "beta"], "{rows:?}");
        assert!(
            rows.contains(&("setup", "setup", "iPad only")),
            "the other session keeps its own row: {rows:?}"
        );
        let (peer, _) =
            crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("setup peer: {e}"));
        this.terminal.core_test_backend = Some(peer);
        this.open_herdr2_row(
            &row.host,
            &row.boot,
            row.generation,
            CoreRow::Space("setup".into()),
            cx,
        )
        .unwrap_or_else(|e| panic!("open setup: {e}"));
    });
    wait_for(&view, cx, "the setup session", |this| {
        this.shell.herdr2.selected_session("pro-dev") == "setup" && this.terminal.live.connected
    });
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
    let _ = std::fs::remove_dir_all(root);
}
