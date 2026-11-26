//! Ended panes out of a space, as the official app's `remove_pane`,
//! `remove_inner` and `collapse` take them: the pane, its share, the
//! containers it leaves empty or alone, the tab it leaves empty; and the
//! tabs that are not plain, which stay whole.
use super::herdr_doc_prune::{plain, prune_space};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn block(id: &str, terminal: &str) -> Value {
    json!({"id": id, "type": "block", "content": {"terminal": terminal}})
}

fn row(id: &str, children: Vec<Value>) -> Value {
    json!({"id": id, "type": "row", "children": children})
}

fn space(tabs: Vec<Value>) -> Value {
    json!({"id": "sp1", "name": "env", "folder": "", "tabs": tabs, "newer": {"kept": 1}})
}

fn tab(id: &str, root: Value) -> Value {
    json!({"id": id, "name": "", "root": root})
}

fn ended(terminals: &[&str]) -> BTreeSet<String> {
    terminals.iter().map(|t| (*t).to_owned()).collect()
}

#[test]
fn live_env_space_loses_its_ended_panes_and_tab() {
    // The owner's `env` space on 2026-11-26 (ids shortened): t37 and t10
    // live; t3, t34 exited and t31 killed.
    let before = space(vec![
        tab(
            "tbS",
            row("tbS", vec![block("p95", "t3"), block("p18", "t37")]),
        ),
        tab(
            "tbY",
            row("tbY", vec![block("pd3", "t10"), block("p49", "t34")]),
        ),
        tab("tb33", row("tb33", vec![block("p2e", "t31")])),
    ]);
    let after = prune_space(&before, &ended(&["t3", "t31", "t34"]));
    // The share of a removed pane goes to its neighbour (1 + 1).
    let mut t37 = block("p18", "t37");
    t37["size"] = json!({"weight": 2.0});
    let mut t10 = block("pd3", "t10");
    t10["size"] = json!({"weight": 2.0});
    let expected = space(vec![
        tab("tbS", row("tbS", vec![t37])),
        tab("tbY", row("tbY", vec![t10])),
    ]);
    assert_eq!(after, expected);
    // Byte for byte where nothing changed: names, order, unknown keys.
    assert_eq!(after["newer"], json!({"kept": 1}));
}

#[test]
fn last_pane_takes_its_tab_and_last_tab_leaves_none() {
    let before = space(vec![tab("tb1", row("tb1", vec![block("p1", "t1")]))]);
    let after = prune_space(&before, &ended(&["t1"]));
    assert_eq!(after["tabs"], json!([]));
}

#[test]
fn live_terminal_keeps_its_pane() {
    // The negative control: nothing ended, nothing changes.
    let before = space(vec![tab(
        "tb1",
        row("tb1", vec![block("p1", "t1"), block("p2", "t2")]),
    )]);
    assert_eq!(prune_space(&before, &ended(&[])), before);
    assert_eq!(prune_space(&before, &ended(&["t9"])), before);
}

#[test]
fn share_goes_to_the_pane_before_else_the_next() {
    let sized = |id: &str, terminal: &str, weight: f64| {
        let mut b = block(id, terminal);
        b["size"] = json!({"weight": weight});
        b
    };
    let before = space(vec![tab(
        "tb1",
        row(
            "tb1",
            vec![
                sized("p1", "t1", 0.25),
                sized("p2", "t2", 0.5),
                sized("p3", "t3", 0.25),
            ],
        ),
    )]);
    // The middle pane: its share goes to the one before it.
    let middle = prune_space(&before, &ended(&["t2"]));
    assert_eq!(
        middle["tabs"][0]["root"]["children"],
        json!([sized("p1", "t1", 0.75), sized("p3", "t3", 0.25)])
    );
    // The first pane: to the next, which slid into its place.
    let first = prune_space(&before, &ended(&["t1"]));
    assert_eq!(
        first["tabs"][0]["root"]["children"],
        json!([sized("p2", "t2", 0.75), sized("p3", "t3", 0.25)])
    );
}

#[test]
fn px_sized_pane_gives_and_takes_no_share() {
    let mut fixed = block("p1", "t1");
    fixed["size"] = json!({"px": 300});
    let before = space(vec![tab(
        "tb1",
        row("tb1", vec![fixed.clone(), block("p2", "t2")]),
    )]);
    let after = prune_space(&before, &ended(&["t2"]));
    assert_eq!(after["tabs"][0]["root"]["children"], json!([fixed]));
}

#[test]
fn container_left_alone_gives_way_to_its_child_with_its_size() {
    let mut column = json!({"id": "c1", "type": "column", "size": {"weight": 3.0},
        "children": [block("p2", "t2"), block("p3", "t3")]});
    let before = space(vec![tab(
        "tb1",
        row("tb1", vec![block("p1", "t1"), column.clone()]),
    )]);
    let after = prune_space(&before, &ended(&["t3"]));
    // p2 took p3's share (2.0), then took the column's place and size.
    let mut p2 = block("p2", "t2");
    p2["size"] = json!({"weight": 3.0});
    assert_eq!(
        after["tabs"][0]["root"]["children"],
        json!([block("p1", "t1"), p2])
    );
    // A container left empty vanishes; the root row keeps its id.
    column["children"] = json!([block("p2", "t2")]);
    let before = space(vec![tab(
        "tb1",
        row("tb1", vec![block("p1", "t1"), column]),
    )]);
    let after = prune_space(&before, &ended(&["t2"]));
    assert_eq!(after["tabs"][0]["root"]["id"], json!("tb1"));
    assert_eq!(
        after["tabs"][0]["root"]["children"],
        json!([block("p1", "t1")])
    );
}

#[test]
fn tab_that_is_not_plain_stays_whole() {
    let with_mod = row(
        "tb1",
        vec![
            block("p1", "t1"),
            json!({"id": "pm", "type": "block", "content": {"mod": "notes", "props": {}}}),
        ],
    );
    let stack = json!({"id": "tb2", "type": "stack", "children": [block("p2", "t2")]});
    let mut floating = row("tb3", vec![block("p3", "t3")]);
    floating["floating"] = json!([]);
    for root in [with_mod, stack, floating] {
        assert!(!plain(&root), "{root}");
        let before = space(vec![tab("tbX", root)]);
        assert_eq!(
            prune_space(&before, &ended(&["t1", "t2", "t3"])),
            before,
            "kept whole"
        );
    }
}

#[test]
fn tab_whose_root_is_one_ended_block_goes() {
    let before = space(vec![
        tab("tb1", block("tb1", "t1")),
        tab("tb2", block("tb2", "t2")),
    ]);
    let after = prune_space(&before, &ended(&["t1"]));
    assert_eq!(after["tabs"], json!([tab("tb2", block("tb2", "t2"))]));
}
