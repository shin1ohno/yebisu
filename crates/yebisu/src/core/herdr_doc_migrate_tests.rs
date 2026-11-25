//! The one-time move of the iPad's saved `default` tabs into the document
//! (#144 D5, Design G, AC12): one new space "iPad", a backup of the iPad
//! file kept once, and never a second migration.
use super::*;
use crate::{
    core::{
        Session,
        herdr_doc::{MIGRATED_SPACE, MigratedTab, apply, check_write, parse_view},
        store::{self, Saved, Tab, Workspace},
    },
    core_mock::Host,
};
use serde_json::{Value, json};
use sheprd_backend::TerminalId;

const BOUND: Duration = Duration::from_secs(10);
const HOST: &str = "pro-dev";

fn wait_until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + BOUND;
    while !done() {
        assert!(Instant::now() < deadline, "{what} not within {BOUND:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn root(test: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("yebisu-migrate-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

fn id(address: &str) -> TerminalId {
    TerminalId::new(address.to_owned()).unwrap_or_else(|e| panic!("{address}: {e}"))
}

/// The iPad file of an earlier build: one split tab (t1 over t2).
fn save_ipad_tabs(root: &std::path::Path) {
    let workspace = Workspace {
        created: 1,
        tabs: vec![Tab {
            id: "t1".into(),
            label: "Shell".into(),
            layout: Saved {
                ids: vec![id("t1"), id("t2")],
                active: 1,
                vertical: true,
            },
        }],
        ..Workspace::default()
    };
    store::save_workspace(Some(root), HOST, "default", &workspace)
        .unwrap_or_else(|e| panic!("save: {e}"));
}

fn connect(host: &Host, root: &std::path::Path) -> (Session, Arc<Mutex<Live>>) {
    let (backend, _) = host
        .recorded(false)
        .unwrap_or_else(|e| panic!("mock core: {e}"));
    let inbox = Arc::new(Mutex::new(Live::default()));
    let session = Session::start_doc(
        HOST.into(),
        1,
        inbox.clone(),
        move || Ok(backend),
        Some(root.to_path_buf()),
        crate::shell::single_pane::Mailbox::detached(),
        DocPlan {
            source: DocSource::Core,
            period: Duration::from_millis(200),
        },
    )
    .unwrap_or_else(|e| panic!("start: {e}"));
    (session, inbox)
}

fn spaces(host: &Host) -> Vec<Value> {
    host.doc().doc().0["spaces"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn saved(root: &std::path::Path) -> Workspace {
    store::load_workspace(Some(root), HOST, "default").unwrap_or_else(|e| panic!("load: {e}"))
}

#[test]
fn migrates_once_into_ipad_space() {
    let root = root("once");
    save_ipad_tabs(&root);
    let original = std::fs::read(root.join("core-70726f2d646576-default.json"))
        .unwrap_or_else(|e| panic!("read: {e}"));
    let host = Host::default();
    let (session, _) = connect(&host, &root);
    wait_until("the iPad space", || spaces(&host).len() == 1);
    let space = &spaces(&host)[0];
    assert_eq!(space["name"], json!(MIGRATED_SPACE));
    assert_eq!(space["folder"], json!(""));
    let tab = &space["tabs"][0];
    assert_eq!(tab["root"]["id"], tab["id"]);
    assert_eq!(
        tab["root"]["type"],
        json!("column"),
        "the split was stacked"
    );
    let terminals = tab["root"]["children"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|block| block["content"]["terminal"].clone())
        .collect::<Vec<_>>();
    assert_eq!(terminals, [json!("t1"), json!("t2")]);
    // The file records the version it moved in at and keeps no tab of it.
    wait_until("the migration recorded", || saved(&root).migrated.is_some());
    let file = saved(&root);
    assert!(
        file.tabs.is_empty(),
        "the moved tabs are the document's now"
    );
    assert_eq!(file.doc_view.space.as_deref(), space["id"].as_str());
    // The backup is the file as the earlier build left it.
    let backup = std::fs::read(root.join("core-70726f2d646576-default.v2.json"))
        .unwrap_or_else(|e| panic!("backup: {e}"));
    assert_eq!(backup, original);
    assert_eq!(host.doc().sets().len(), 1, "one write");
    drop(session);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn second_connect_does_not_migrate() {
    let root = root("second");
    save_ipad_tabs(&root);
    let host = Host::default();
    let (first, _) = connect(&host, &root);
    wait_until("the migration recorded", || saved(&root).migrated.is_some());
    drop(first);
    // A tab of the iPad's own again, with a terminal no document names (as
    // if the document had been unreadable): only `migrated` keeps it here.
    let (other, _) = host.recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    let third = other
        .core()
        .run(&sheprd_core_client::wire::Run {
            keep: true,
            ..Default::default()
        })
        .unwrap_or_else(|e| panic!("run: {e}"));
    let mut workspace = saved(&root);
    workspace.tabs.push(Tab {
        id: third.address.clone(),
        label: "Shell".into(),
        layout: Saved {
            ids: vec![id(&third.address)],
            ..Saved::default()
        },
    });
    store::save_workspace(Some(&root), HOST, "default", &workspace)
        .unwrap_or_else(|e| panic!("save: {e}"));
    let sets = host.doc().sets().len();
    let (second, inbox) = connect(&host, &root);
    wait_until("the second connection drew", || {
        inbox.lock().is_ok_and(|live| live.connected)
    });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(spaces(&host).len(), 1, "no second iPad space");
    assert_eq!(host.doc().sets().len(), sets, "nothing written");
    drop(second);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn crash_after_write_marks_without_second_space() {
    // The write landed but the iPad never recorded it: the next connection
    // finds the terminals in the document and records the migration only.
    let root = root("crash");
    save_ipad_tabs(&root);
    let host = Host::default();
    host.doc().set_silent(
        json!({"spaces": [{"id": "spold", "name": "iPad", "folder": "",
        "tabs": [{"id": "tbold", "name": "", "root": {"id": "tbold", "type": "column", "children": [
            {"id": "p1", "type": "block", "content": {"terminal": "t1"}},
            {"id": "p2", "type": "block", "content": {"terminal": "t2"}}]}}]}]}),
    );
    let (session, _) = connect(&host, &root);
    wait_until("the migration recorded", || saved(&root).migrated.is_some());
    assert_eq!(spaces(&host).len(), 1);
    assert!(host.doc().sets().is_empty());
    drop(session);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn backup_written_once() {
    let root = root("backup");
    save_ipad_tabs(&root);
    let file = root.join("core-70726f2d646576-default.json");
    let backup = root.join("core-70726f2d646576-default.v2.json");
    let first = std::fs::read(&file).unwrap_or_else(|e| panic!("read: {e}"));
    store::backup_once(Some(&root), HOST).unwrap_or_else(|e| panic!("backup: {e}"));
    std::fs::write(&file, b"{}").unwrap_or_else(|e| panic!("write: {e}"));
    store::backup_once(Some(&root), HOST).unwrap_or_else(|e| panic!("again: {e}"));
    assert_eq!(
        std::fs::read(&backup).ok(),
        Some(first),
        "never overwritten"
    );
    // No file to copy is no error, and no backup.
    let empty = self::root("backup-empty");
    store::backup_once(Some(&empty), HOST).unwrap_or_else(|e| panic!("empty: {e}"));
    assert!(!empty.join("core-70726f2d646576-default.v2.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn backup_is_not_a_session() {
    let root = root("not-a-session");
    save_ipad_tabs(&root);
    store::backup_once(Some(&root), HOST).unwrap_or_else(|e| panic!("backup: {e}"));
    let names = store::sessions(Some(&root), HOST).unwrap_or_else(|e| panic!("sessions: {e}"));
    assert_eq!(names, ["default"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn migrate_intent_shape_and_idempotency() {
    let tabs = vec![
        MigratedTab {
            tab_id: "tba".into(),
            panes: vec![("pa".into(), "t1".into()), ("pb".into(), "t2".into())],
            vertical: false,
        },
        MigratedTab {
            tab_id: "tbb".into(),
            panes: vec![("pc".into(), "t5".into())],
            vertical: true,
        },
    ];
    let intent = Intent::Migrate {
        space_id: "spipad".into(),
        tabs,
    };
    let original = json!({"spaces": [{"id": "spa", "tabs": [{"id": "tbx", "root": {"id": "tbx",
        "type": "row", "children": [{"id": "px", "type": "block", "content": {"terminal": "t5"}}]}}]}]});
    let Ok(Applied::Changed(next)) = apply(&original, &intent) else {
        panic!("migrate changed nothing");
    };
    let space = &next["spaces"][1];
    assert_eq!(
        *space,
        json!({"id": "spipad", "name": "iPad", "folder": "", "tabs": [{"id": "tba", "name": "",
            "root": {"id": "tba", "type": "row", "children": [
                {"id": "pa", "type": "block", "content": {"terminal": "t1"}},
                {"id": "pb", "type": "block", "content": {"terminal": "t2"}}]}}]}),
        "t5 is in the document already: its tab is not moved"
    );
    check_write(&original, &next, &intent).unwrap_or_else(|e| panic!("check: {e}"));
    parse_view(&next).unwrap_or_else(|e| panic!("view: {e}"));
    assert_eq!(apply(&next, &intent).ok(), Some(Applied::Unchanged));
    // On an empty document too, and nothing to move is nothing to write.
    assert!(matches!(
        apply(&Value::Null, &intent),
        Ok(Applied::Changed(_))
    ));
    let empty = Intent::Migrate {
        space_id: "spnone".into(),
        tabs: Vec::new(),
    };
    assert_eq!(apply(&Value::Null, &empty).ok(), Some(Applied::Unchanged));
}
