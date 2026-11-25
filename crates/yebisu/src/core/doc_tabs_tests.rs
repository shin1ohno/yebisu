//! The drawn tabs of a `default` session reading `herdr-app` (#144 D2,
//! AC1 and AC13, owner answer D2-1): the selected space's tabs, the
//! iPad-only tabs, the first two terminals of a tree yebisu cannot draw, and
//! the iPad file's own view of the document.
use super::{
    DocRows,
    doc_tabs::DocTabs,
    herdr_doc::parse_view,
    herdr_doc_sync::{DocSnapshot, Read},
    store::{self, DocView, Saved, Tab, Workspace},
};
use serde_json::Value;
use sheprd_backend::TerminalId;
use std::collections::BTreeMap;

/// The space of `tree-three-panes.json`.
const SP1: &str = "sp00000000000000000000000000000005";
/// The two spaces of `two-spaces.json`.
const B1: &str = "sp00000000000000000000000000000015";
const B2: &str = "sp00000000000000000000000000000016";

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<Value>(include_str!(concat!(
            "../../fixtures/herdr-app/",
            $name,
            ".json"
        )))
        .unwrap_or_else(|e| panic!("{}: {e}", $name))
    };
}

fn snapshot(value: &Value, live: &[&str]) -> DocSnapshot {
    DocSnapshot {
        version: 3,
        read: Read::Readable(parse_view(value).unwrap_or_else(|e| panic!("view: {e}"))),
        live: live.iter().map(|t| (*t).to_owned()).collect(),
        names: BTreeMap::new(),
        stats: Default::default(),
    }
}

fn id(address: &str) -> TerminalId {
    TerminalId::new(address.to_owned()).unwrap_or_else(|e| panic!("{address}: {e}"))
}

fn local(tab: &str, ids: &[&str]) -> Tab {
    Tab {
        id: tab.into(),
        label: tab.into(),
        layout: Saved {
            ids: ids.iter().map(|t| id(t)).collect(),
            ..Saved::default()
        },
    }
}

fn drawn(workspace: &Workspace) -> Vec<(String, Vec<String>)> {
    workspace
        .tabs
        .iter()
        .map(|tab| {
            let ids = tab.layout.ids.iter().map(|id| id.as_str().to_owned());
            (tab.label.clone(), ids.collect())
        })
        .collect()
}

fn pair(label: &str, ids: &[&str]) -> (String, Vec<String>) {
    (label.into(), ids.iter().map(|t| (*t).to_owned()).collect())
}

#[test]
fn tab_strip_is_selected_space_tabs() {
    let mut workspace = Workspace::default();
    let mut doc = DocTabs::default();
    assert!(doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2"]),
        &mut workspace
    ));
    // No iPad-only tabs: the first document space (owner answer D2-1 (4)).
    assert!(doc.drawing_space());
    assert_eq!(workspace.doc_view.space.as_deref(), Some(B1));
    assert_eq!(
        drawn(&workspace),
        [pair("1", &["t1"]), pair("logs", &["t2"])]
    );
    let catalog = doc.catalog(&workspace).unwrap_or_else(|| panic!("catalog"));
    assert_eq!(catalog.selected.as_deref(), Some(B1));
    assert_eq!(catalog.local_tabs, 0);
    let DocRows::Readable(spaces) = catalog.state else {
        panic!("readable");
    };
    let rows = spaces
        .iter()
        .map(|s| (s.name.as_str(), s.auto_name, s.tabs.len()))
        .collect::<Vec<_>>();
    // An unnamed space shows its folder's last component until the host
    // names it (decision 9).
    assert_eq!(rows, [("Build", false, 2), ("beta", true, 1)]);
}

#[test]
fn ipad_only_tabs_are_drawn_first_when_there_are_any() {
    let mut workspace = Workspace {
        tabs: vec![local("t9", &["t9"])],
        ..Workspace::default()
    };
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2", "t9"]),
        &mut workspace,
    );
    assert!(!doc.drawing_space());
    assert_eq!(drawn(&workspace), [pair("t9", &["t9"])]);
    assert_eq!(doc.catalog(&workspace).map(|c| c.selected), Some(None));
}

#[test]
fn document_terminals_leave_the_ipad_only_tabs() {
    // R4: a terminal the document names is never also an iPad-only tab.
    let mut workspace = Workspace {
        tabs: vec![local("t1", &["t1", "t9"]), local("t2", &["t2"])],
        ..Workspace::default()
    };
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2", "t9"]),
        &mut workspace,
    );
    assert_eq!(drawn(&workspace), [pair("t1", &["t9"])]);
    assert_eq!(doc.catalog(&workspace).map(|c| c.local_tabs), Some(1));
}

#[test]
fn selecting_a_space_and_back_keeps_the_ipad_only_tabs() {
    let mut workspace = Workspace {
        tabs: vec![local("a", &["t8"]), local("b", &["t9"])],
        active: 1,
        ..Workspace::default()
    };
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2", "t8", "t9"]),
        &mut workspace,
    );
    doc.select(Some(B1.into()), &mut workspace);
    assert_eq!(
        drawn(&workspace),
        [pair("1", &["t1"]), pair("logs", &["t2"])]
    );
    workspace.active = 1;
    doc.note_tab(&mut workspace);
    // The file keeps the iPad-only tabs and the iPad's choice of tab.
    let saved = doc.saved(&workspace);
    assert_eq!(drawn(&saved), [pair("a", &["t8"]), pair("b", &["t9"])]);
    assert_eq!(saved.doc_view.space.as_deref(), Some(B1));
    assert_eq!(
        saved.doc_view.tabs.get(B1).map(String::as_str),
        Some("tb00000000000000000000000000000016")
    );
    doc.select(None, &mut workspace);
    assert_eq!(drawn(&workspace), [pair("a", &["t8"]), pair("b", &["t9"])]);
    assert_eq!(workspace.active, 1);
    // Back in the space, the tab chosen before is drawn again.
    doc.select(Some(B1.into()), &mut workspace);
    assert_eq!(workspace.active, 1);
}

#[test]
fn a_tab_without_a_live_terminal_is_not_drawn() {
    let mut workspace = Workspace::default();
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2"]),
        &mut workspace,
    );
    doc.select(Some(B2.into()), &mut workspace);
    assert!(doc.drawing_space());
    assert!(workspace.tabs.is_empty());
}

#[test]
fn three_pane_tab_shows_first_two() {
    // AC13: the first two terminals in depth-first order; the tree is not
    // changed (D2 writes nothing).
    let mut workspace = Workspace::default();
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("tree-three-panes"), &["t1", "t2", "t3"]),
        &mut workspace,
    );
    assert_eq!(drawn(&workspace), [pair("1", &["t1", "t2"])]);
    // The third is reached by focusing it: its tab then draws it alone.
    assert!(doc.reveal("t3", &mut workspace));
    assert_eq!(drawn(&workspace), [pair("1", &["t3"])]);
    doc.note_tab(&mut workspace);
    doc.select(Some(SP1.into()), &mut workspace);
    assert_eq!(drawn(&workspace), [pair("1", &["t1", "t2"])]);
}

#[test]
fn stack_mod_float_tabs_show_first_two() {
    for (value, want) in [
        (fixture!("tree-stack"), vec!["t1", "t2"]),
        (fixture!("tree-float"), vec!["t1", "t2"]),
        (fixture!("tree-mod"), vec!["t1"]),
        (fixture!("tree-tab-agent"), vec!["t1"]),
    ] {
        let mut workspace = Workspace::default();
        let mut doc = DocTabs::default();
        doc.take(snapshot(&value, &["t1", "t2"]), &mut workspace);
        assert_eq!(drawn(&workspace), [pair("1", &want)], "{value}");
    }
}

#[test]
fn a_two_pane_tree_keeps_its_direction() {
    let mut workspace = Workspace::default();
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("tree-px"), &["t1", "t2", "t3", "t4"]),
        &mut workspace,
    );
    let vertical = workspace
        .tabs
        .iter()
        .map(|tab| tab.layout.vertical)
        .collect::<Vec<_>>();
    assert_eq!(vertical, [true, false]);
}

#[test]
fn reveal_draws_the_space_that_holds_the_address() {
    let mut workspace = Workspace {
        tabs: vec![local("a", &["t9"])],
        ..Workspace::default()
    };
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2", "t9"]),
        &mut workspace,
    );
    assert!(doc.reveal("t2", &mut workspace));
    assert_eq!(workspace.doc_view.space.as_deref(), Some(B1));
    assert_eq!(workspace.tabs[workspace.active].label, "logs");
    // An iPad-only terminal draws the iPad-only tabs again.
    assert!(doc.reveal("t9", &mut workspace));
    assert!(!doc.drawing_space());
    assert!(
        !doc.reveal("t7", &mut workspace),
        "an unknown terminal changes nothing"
    );
}

#[test]
fn unreadable_document_draws_the_ipad_only_tabs() {
    let mut workspace = Workspace {
        tabs: vec![local("a", &["t1"])],
        doc_view: DocView {
            space: Some(B1.into()),
            tabs: BTreeMap::new(),
        },
        ..Workspace::default()
    };
    let mut doc = DocTabs::default();
    let unreadable = DocSnapshot {
        read: Read::Unreadable,
        ..snapshot(&Value::Null, &["t1"])
    };
    assert!(doc.take(unreadable, &mut workspace));
    assert!(!doc.drawing_space());
    assert_eq!(drawn(&workspace), [pair("a", &["t1"])]);
    let catalog = doc.catalog(&workspace).unwrap_or_else(|| panic!("catalog"));
    assert_eq!(catalog.state, DocRows::Unreadable);
}

#[test]
fn a_failed_read_keeps_what_is_drawn() {
    let mut workspace = Workspace::default();
    let mut doc = DocTabs::default();
    doc.take(
        snapshot(&fixture!("two-spaces"), &["t1", "t2"]),
        &mut workspace,
    );
    let failed = DocSnapshot {
        read: Read::Unavailable("lost".into()),
        ..snapshot(&Value::Null, &[])
    };
    assert!(!doc.take(failed, &mut workspace));
    assert_eq!(drawn(&workspace).len(), 2);
}

#[test]
fn a_space_removed_elsewhere_falls_back() {
    let mut workspace = Workspace {
        doc_view: DocView {
            space: Some("spgone".into()),
            tabs: BTreeMap::new(),
        },
        ..Workspace::default()
    };
    let mut doc = DocTabs::default();
    doc.take(snapshot(&fixture!("tree-mod"), &["t1"]), &mut workspace);
    assert_eq!(
        workspace.doc_view.space.as_deref(),
        Some("sp00000000000000000000000000000007")
    );
}

#[test]
fn doc_view_round_trips_through_the_file() {
    let root = std::env::temp_dir().join(format!("yebisu-doc-view-{}", std::process::id()));
    let workspace = Workspace {
        created: 5,
        tabs: vec![local("a", &["t9"])],
        active: 0,
        doc_view: DocView {
            space: Some(B1.into()),
            tabs: BTreeMap::from([(B1.to_owned(), "tbx".to_owned())]),
        },
        migrated: None,
    };
    store::save_workspace(Some(&root), "pro-dev", "default", &workspace)
        .unwrap_or_else(|e| panic!("save: {e}"));
    let loaded = store::load_workspace(Some(&root), "pro-dev", "default")
        .unwrap_or_else(|e| panic!("load: {e}"));
    assert_eq!(loaded.doc_view, workspace.doc_view);
    assert_eq!(drawn(&loaded), [pair("a", &["t9"])]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_file_without_or_with_a_broken_doc_view_still_loads() {
    let root = std::env::temp_dir().join(format!("yebisu-doc-view-old-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("root: {e}"));
    let path = root.join("core-70726f2d646576-default.json");
    for doc_view in [
        "",
        r#","doc_view":7"#,
        r#","doc_view":{"space":3,"tabs":[]}"#,
    ] {
        let text = format!(
            r#"{{"version":2,"host":"pro-dev","session":"default","created":1,"tabs":[{{"id":"a","label":"a","ids":["t9"],"active":0,"vertical":false}}],"active":0{doc_view}}}"#
        );
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("write: {e}"));
        let loaded = store::load_workspace(Some(&root), "pro-dev", "default")
            .unwrap_or_else(|e| panic!("{doc_view}: {e}"));
        assert_eq!(loaded.doc_view, DocView::default(), "{doc_view}");
        assert_eq!(loaded.tabs.len(), 1);
    }
    let _ = std::fs::remove_dir_all(root);
}
