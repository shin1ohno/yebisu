use super::*;
use crate::core::store::Tab;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap_or_else(|e| panic!("id: {e}"))
}

fn saved(ids: &[&str], active: usize) -> Saved {
    Saved {
        ids: ids.iter().map(|name| id(name)).collect(),
        active,
        vertical: false,
    }
}

fn tab(name: &str, ids: &[&str]) -> Tab {
    Tab {
        id: name.into(),
        label: name.into(),
        layout: saved(ids, 0),
    }
}

#[test]
fn ended_pane_hands_use_to_its_neighbour() {
    let mut layout = saved(&["t1", "t2"], 1);
    remove_pane(&mut layout, 1);
    assert_eq!(layout.ids, vec![id("t1")]);
    assert_eq!(layout.active, 0);

    let mut layout = saved(&["t1", "t2"], 0);
    remove_pane(&mut layout, 0);
    assert_eq!(layout.ids, vec![id("t2")]);
    assert_eq!(layout.active, 0);

    // A pane out of use leaves the one in use in use.
    let mut layout = saved(&["t1", "t2", "t3"], 2);
    remove_pane(&mut layout, 0);
    assert_eq!(layout.ids, vec![id("t2"), id("t3")]);
    assert_eq!(layout.active, 1);
    remove_pane(&mut layout, 7);
    assert_eq!(layout.ids.len(), 2);
}

#[test]
fn finished_terminal_closes_its_pane_then_its_empty_tab() {
    let mut workspace = Workspace {
        created: 1,
        tabs: vec![
            tab("a", &["t1", "t2"]),
            tab("b", &["t3"]),
            tab("c", &["t4"]),
        ],
        active: 1,
        ..Workspace::default()
    };
    assert!(remove_terminal(&mut workspace, &id("t2")));
    assert_eq!(workspace.tabs[0].layout.ids, vec![id("t1")]);
    assert_eq!(workspace.active, 1);
    // The tab in use closes; the one sliding into its place is in use.
    assert!(remove_terminal(&mut workspace, &id("t3")));
    assert_eq!(
        workspace
            .tabs
            .iter()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "c"]
    );
    assert_eq!(workspace.active, 1);
    // A tab before the one in use closes; the same tab stays in use.
    assert!(remove_terminal(&mut workspace, &id("t1")));
    assert_eq!(workspace.tabs[workspace.active].id, "c");
    // The last tab closes: no tabs, nothing in use.
    assert!(remove_terminal(&mut workspace, &id("t4")));
    assert!(workspace.tabs.is_empty());
    assert_eq!(workspace.active, 0);
    assert!(!remove_terminal(&mut workspace, &id("t4")));
}

#[test]
fn only_a_finished_refusal_is_a_finished_terminal() {
    use sheprd_core_client::{Error, Method};
    let refused = |kind: Option<&str>| Error::Refused {
        method: Method::TerminalAttach,
        kind: kind.map(str::to_owned),
        message: "t2 has finished".into(),
    };
    assert!(finished(&refused(Some("finished"))));
    assert!(!finished(&refused(Some("not_found"))));
    assert!(!finished(&refused(None)));
    assert!(!finished(&Error::Lost));
}
