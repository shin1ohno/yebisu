//! The `herdr-app` document model (issue #144 D1): every fixture reads and
//! writes back byte for byte, intents change only their own path, the
//! pre-write gates each refuse what they guard, and the mock core keeps
//! herdr-core's version check and events.
use super::herdr_doc::*;
use crate::core_mock::Host;
use serde_json::{Value, json};
use sheprd_core_client::{Error as CoreError, event::Notice, wire::Run};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

type Outcome = Result<(), Box<dyn std::error::Error>>;

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<Value>(include_str!(concat!(
            "../../fixtures/herdr-app/",
            $name,
            ".json"
        )))?
    };
}

/// Every fixture in the directory, so a newly added one (a real document
/// from an official-app release) is covered without a code change.
fn fixtures() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/herdr-app");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned();
            found.push((name, std::fs::read_to_string(&path)?));
        }
    }
    found.sort();
    Ok(found)
}

fn readable() -> Result<Vec<(String, Value)>, Box<dyn std::error::Error>> {
    let mut found = Vec::new();
    for (name, raw) in fixtures()? {
        if !name.starts_with("unreadable-") {
            found.push((name, serde_json::from_str(&raw)?));
        }
    }
    Ok(found)
}

/// The fixture's text without the whitespace between tokens.
fn minify(raw: &str) -> String {
    let (mut out, mut quoted, mut escaped) = (String::new(), false, false);
    for c in raw.chars() {
        if quoted {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
        } else if c == '"' {
            quoted = true;
            out.push(c);
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

/// Every path where `a` and `b` differ, as `spaces[0].tabs[1].name`.
fn paths(a: &Value, b: &Value) -> Vec<String> {
    fn walk(a: &Value, b: &Value, at: &str, out: &mut Vec<String>) {
        let key = |k: &str| {
            if at.is_empty() {
                k.to_owned()
            } else {
                format!("{at}.{k}")
            }
        };
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for (k, v) in x {
                    match y.get(k) {
                        Some(w) => walk(v, w, &key(k), out),
                        None => out.push(key(k)),
                    }
                }
                out.extend(y.keys().filter(|k| !x.contains_key(*k)).map(|k| key(k)));
            }
            (Value::Array(x), Value::Array(y)) => {
                for i in 0..x.len().max(y.len()) {
                    match (x.get(i), y.get(i)) {
                        (Some(v), Some(w)) => walk(v, w, &format!("{at}[{i}]"), out),
                        _ => out.push(format!("{at}[{i}]")),
                    }
                }
            }
            _ if a != b => out.push(at.to_owned()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(a, b, "", &mut out);
    out
}

fn changed(applied: Applied) -> Result<Value, String> {
    match applied {
        Applied::Changed(value) => Ok(value),
        Applied::Unchanged => Err("the intent changed nothing".into()),
    }
}

/// The node with `id` anywhere under `value`.
fn node<'a>(value: &'a Value, id: &str) -> Option<&'a Value> {
    if value.get("id").and_then(Value::as_str) == Some(id) && value.get("type").is_some() {
        return Some(value);
    }
    match value {
        Value::Object(map) => map.values().find_map(|v| node(v, id)),
        Value::Array(items) => items.iter().find_map(|v| node(v, id)),
        _ => None,
    }
}

const SP1: &str = "sp00000000000000000000000000000001";
const TB1: &str = "tb00000000000000000000000000000001";
const TB2: &str = "tb00000000000000000000000000000002";
const P1: &str = "p00000000000000000000000000000001";

fn new_tab(space: &str, terminal: &str) -> Intent {
    Intent::NewTab {
        space: space.into(),
        tab_id: "tbnew".into(),
        pane_id: "pnew".into(),
        terminal: terminal.into(),
    }
}

fn new_space(terminal: &str) -> Intent {
    Intent::NewSpace {
        space_id: "spnew".into(),
        folder: "/Users/owner/src/beta".into(),
        tab_id: "tbnew".into(),
        pane_id: "pnew".into(),
        terminal: terminal.into(),
    }
}

fn split(terminal: &str, axis: Axis) -> Intent {
    Intent::Split {
        terminal: terminal.into(),
        axis,
        new_pane: "pnew".into(),
        new_container: "snew".into(),
        new_terminal: "t9".into(),
    }
}

fn rename_space(id: &str, name: &str) -> Intent {
    Intent::RenameSpace {
        id: id.into(),
        name: name.into(),
    }
}

fn rename_tab(id: &str, name: &str) -> Intent {
    Intent::RenameTab {
        id: id.into(),
        name: name.into(),
    }
}

#[test]
fn roundtrip_is_byte_identical_for_every_fixture() -> Outcome {
    let all = fixtures()?;
    // Positive control: the directory listing found the fixtures.
    assert!(all.len() >= 17, "only {} fixtures found", all.len());
    for (name, raw) in all {
        let value: Value = serde_json::from_str(&raw)?;
        // What yebisu would send back for a document it read unchanged.
        assert_eq!(serde_json::to_string(&value)?, minify(&raw), "{name}");
        if !name.starts_with("unreadable-") {
            parse_view(&value).map_err(|e| format!("{name}: {e}"))?;
        }
    }
    Ok(())
}

#[test]
fn key_order_survives_a_read() -> Outcome {
    // Byte identity needs serde_json's `preserve_order` (enabled by gpui).
    let raw = r#"{"z":1,"spaces":[{"tabs":[],"id":"b","a":2}],"m":null}"#;
    assert_eq!(
        serde_json::to_string(&serde_json::from_str::<Value>(raw)?)?,
        raw
    );
    Ok(())
}

#[test]
fn empty_documents_have_no_spaces() -> Outcome {
    for value in [
        Value::Null,
        json!({}),
        json!({"spaces": []}),
        json!({"spaces": null}),
    ] {
        assert_eq!(parse_view(&value)?, View::default(), "{value}");
    }
    Ok(())
}

#[test]
fn unreadable_fixtures_are_rejected() -> Outcome {
    for (name, at) in [
        ("unreadable-child", "spaces[0].tabs[0].root.children[0]"),
        ("unreadable-root", "spaces[0].tabs[0].root.type"),
        ("unreadable-spaces", "spaces"),
        ("unreadable-top", "the top"),
    ] {
        let raw = fixtures()?
            .into_iter()
            .find(|(found, _)| found == name)
            .ok_or(name)?
            .1;
        match parse_view(&serde_json::from_str(&raw)?) {
            Err(DocError::Unreadable(path)) => assert_eq!(path, at, "{name}"),
            other => return Err(format!("{name}: {other:?}").into()),
        }
    }
    Ok(())
}

#[test]
fn shapes_the_official_app_cannot_read_are_rejected() {
    for value in [
        json!("text"),
        json!({"spaces": ["sp"]}),
        json!({"spaces": [{"tabs": {}}]}),
        json!({"spaces": [{"id": 7}]}),
        json!({"spaces": [{"name": false}]}),
        json!({"spaces": [{"tabs": [{"id": "tb", "root": "row"}]}]}),
        json!({"spaces": [{"tabs": [{"root": {"type": 3}}]}]}),
        json!({"spaces": [{"tabs": [{"root": {"type": "row", "children": {}}}]}]}),
        json!({"spaces": [{"tabs": [{"root": {"type": "block", "content": "t1"}}]}]}),
        json!({"spaces": [{"tabs": [{"root": {"type": "row", "size": 4}}]}]}),
        json!({"spaces": [{"tabs": [{"root": {"type": "row", "size": {"px": "4"}}}]}]}),
    ] {
        assert!(
            matches!(parse_view(&value), Err(DocError::Unreadable(_))),
            "{value}"
        );
    }
}

#[test]
fn terminals_are_depth_first_and_drawability_follows_the_tree() -> Outcome {
    let cases = [
        (fixture!("unknown-fields"), 0, vec!["t1"], Drawable::One),
        (
            fixture!("unknown-fields"),
            1,
            vec!["t2", "t3"],
            Drawable::Not,
        ),
        (
            fixture!("core-roundtrip-written"),
            0,
            vec!["t1"],
            Drawable::One,
        ),
        (
            fixture!("tree-three-panes"),
            0,
            vec!["t1", "t2", "t3"],
            Drawable::Not,
        ),
        (fixture!("tree-stack"), 0, vec!["t1", "t2"], Drawable::Not),
        (fixture!("tree-mod"), 0, vec!["t1"], Drawable::Not),
        (fixture!("tree-float"), 0, vec!["t1", "t2"], Drawable::Not),
        (fixture!("tree-tab-agent"), 0, vec!["t1"], Drawable::Not),
        (
            fixture!("tree-px"),
            0,
            vec!["t1", "t2"],
            Drawable::Two { vertical: true },
        ),
        (
            fixture!("tree-px"),
            1,
            vec!["t3", "t4"],
            Drawable::Two { vertical: false },
        ),
        (
            fixture!("new-node-fields"),
            0,
            vec!["t1", "t2"],
            Drawable::Two { vertical: false },
        ),
    ];
    for (value, tab, terminals, drawable) in cases {
        let view = parse_view(&value)?;
        let tab = &view.spaces[0].tabs[tab];
        assert_eq!(tab.terminals, terminals, "{}", tab.id);
        assert_eq!(tab.drawable, drawable, "{}", tab.id);
    }
    let omitted = parse_view(&fixture!("omitted"))?;
    assert_eq!(omitted.spaces.len(), 3);
    assert_eq!(omitted.spaces[0], SpaceView::default());
    assert_eq!(omitted.spaces[2].name, "named");
    for tab in &omitted.spaces[1].tabs {
        assert_eq!((tab.terminals.len(), tab.drawable), (0, Drawable::Not));
    }
    Ok(())
}

#[test]
fn two_terminals_draw_only_under_one_direction() -> Outcome {
    let block = |id: &str, t: &str| json!({"id": id, "type": "block", "content": {"terminal": t}});
    let tab = |root: Value| json!({"spaces": [{"tabs": [{"root": root}]}]});
    let cases = [
        (
            json!({"type": "row", "children": [{"type": "row", "children": [block("a", "t1"), block("b", "t2")]}]}),
            Drawable::Two { vertical: false },
        ),
        (
            json!({"type": "row", "children": [{"type": "column", "children": [block("a", "t1")]}, block("b", "t2")]}),
            Drawable::Two { vertical: false },
        ),
        (
            json!({"type": "column", "children": [{"type": "row", "children": [block("a", "t1"), block("b", "t2")]}]}),
            Drawable::Not,
        ),
        (
            json!({"type": "grid", "children": [block("a", "t1")]}),
            Drawable::Not,
        ),
        (
            json!({"type": "row", "children": [{"type": "block", "content": {"terminal": "t1", "x": 1}}]}),
            Drawable::One,
        ),
    ];
    for (root, drawable) in cases {
        let view = parse_view(&tab(root.clone()))?;
        assert_eq!(view.spaces[0].tabs[0].drawable, drawable, "{root}");
        assert!(!view.spaces[0].tabs[0].terminals.is_empty(), "{root}");
    }
    Ok(())
}

#[test]
fn unknown_fields_survive_every_intent() -> Outcome {
    let original = fixture!("unknown-fields");
    for intent in [
        rename_space(SP1, "Alpha"),
        rename_tab(TB1, "Build"),
        rename_tab(TB2, ""),
        new_tab(SP1, "t9"),
        new_space("t9"),
        split("t1", Axis::Row),
        split("t1", Axis::Column),
    ] {
        let next = changed(apply(&original, &intent)?)?;
        let tab = &next["spaces"][0]["tabs"][0];
        assert_eq!(next["x-top"], json!({"layout": {"gap": 4}}), "{intent:?}");
        assert_eq!(
            next["spaces"][0]["x-space"],
            json!({"kept": true}),
            "{intent:?}"
        );
        assert_eq!(tab["x-tab"], json!([1, 2]), "{intent:?}");
        assert_eq!(tab["root"]["x-node"], json!("row"), "{intent:?}");
        let block = node(tab, P1).ok_or("block p1 is gone")?;
        assert_eq!(block["x-node"], json!("block"), "{intent:?}");
        let float = &next["spaces"][0]["tabs"][1]["root"]["floating"][0];
        assert_eq!(float["x-float"], json!({"x": 10, "y": 20}), "{intent:?}");
        check_write(&original, &next, &intent)?;
    }
    Ok(())
}

#[test]
fn rename_changes_one_path_in_every_fixture() -> Outcome {
    let mut renamed = 0;
    for (name, original) in readable()? {
        let view = parse_view(&original)?;
        for (i, space) in view.spaces.iter().enumerate() {
            let mut targets = vec![(
                rename_space(&space.id, "Renamed"),
                format!("spaces[{i}].name"),
            )];
            for (j, tab) in space.tabs.iter().enumerate() {
                targets.push((
                    rename_tab(&tab.id, "Renamed"),
                    format!("spaces[{i}].tabs[{j}].name"),
                ));
            }
            for (intent, at) in targets {
                let ids_known = match &intent {
                    Intent::RenameSpace { id, .. } | Intent::RenameTab { id, .. } => !id.is_empty(),
                    _ => false,
                };
                if !ids_known {
                    continue;
                }
                let next = changed(apply(&original, &intent)?)?;
                assert_eq!(paths(&original, &next), vec![at], "{name}");
                check_write(&original, &next, &intent).map_err(|e| format!("{name}: {e}"))?;
                renamed += 1;
            }
        }
    }
    assert!(renamed >= 20, "only {renamed} renames were tried");
    Ok(())
}

#[test]
fn rename_empty_is_the_automatic_name_and_names_are_trimmed() -> Outcome {
    let original = fixture!("unknown-fields");
    for (name, want) in [("", ""), ("   ", ""), ("  Build \n", "Build")] {
        let next = changed(apply(&original, &rename_tab(TB2, name))?)?;
        assert_eq!(next["spaces"][0]["tabs"][1]["name"], json!(want));
    }
    let next = changed(apply(&original, &rename_space(SP1, " Alpha "))?)?;
    assert_eq!(next["spaces"][0]["name"], json!("Alpha"));
    Ok(())
}

#[test]
fn rename_to_the_same_name_or_a_missing_id_changes_nothing() -> Outcome {
    let original = fixture!("unknown-fields");
    for intent in [
        rename_tab(TB2, "logs"),
        rename_tab(TB2, " logs "),
        rename_tab(TB1, ""),
        rename_space(SP1, ""),
        rename_space("spmissing", "Alpha"),
        rename_tab("tbmissing", "Build"),
    ] {
        assert_eq!(apply(&original, &intent)?, Applied::Unchanged, "{intent:?}");
    }
    // A space with no `name` key keeps none for the automatic name.
    let omitted = fixture!("omitted");
    let space = "sp00000000000000000000000000000002";
    assert_eq!(
        apply(&omitted, &rename_space(space, ""))?,
        Applied::Unchanged
    );
    let next = changed(apply(&omitted, &rename_space(space, "Named"))?)?;
    assert_eq!(paths(&omitted, &next), vec!["spaces[1].name"]);
    Ok(())
}

#[test]
fn rename_keeps_tree_floating_and_size() -> Outcome {
    // AC13: undrawable trees are never rewritten by a rename.
    for original in [
        fixture!("tree-three-panes"),
        fixture!("tree-float"),
        fixture!("tree-px"),
        fixture!("tree-mod"),
        fixture!("tree-stack"),
        fixture!("tree-tab-agent"),
    ] {
        let view = parse_view(&original)?;
        let tab = &view.spaces[0].tabs[0];
        let next = changed(apply(&original, &rename_tab(&tab.id, "Renamed"))?)?;
        assert_eq!(
            next["spaces"][0]["tabs"][0]["root"],
            original["spaces"][0]["tabs"][0]["root"]
        );
        let space = changed(apply(&original, &rename_space(&view.spaces[0].id, "S"))?)?;
        assert_eq!(space["spaces"][0]["tabs"], original["spaces"][0]["tabs"]);
    }
    Ok(())
}

#[test]
fn new_tab_shape_matches_the_design() -> Outcome {
    let original = fixture!("unknown-fields");
    let next = changed(apply(&original, &new_tab(SP1, "t9"))?)?;
    assert_eq!(
        serde_json::to_string(&next["spaces"][0]["tabs"][2])?,
        r#"{"id":"tbnew","name":"","root":{"id":"tbnew","type":"row","children":[{"id":"pnew","type":"block","content":{"terminal":"t9"}}]}}"#
    );
    assert_eq!(paths(&original, &next), vec!["spaces[0].tabs[2]"]);
    // A space with no `tabs` gets one; an unknown space is an error.
    let bare = json!({"spaces": [{"id": "spa"}, {"id": "spb", "tabs": null}]});
    for space in ["spa", "spb"] {
        let next = changed(apply(&bare, &new_tab(space, "t9"))?)?;
        check_write(&bare, &next, &new_tab(space, "t9"))?;
    }
    assert!(matches!(
        apply(&original, &new_tab("spmissing", "t9")),
        Err(DocError::Missing("space"))
    ));
    Ok(())
}

#[test]
fn new_space_shape_matches_the_design() -> Outcome {
    let tab = json!({"id": "tbnew", "name": "", "root": {"id": "tbnew", "type": "row",
        "children": [{"id": "pnew", "type": "block", "content": {"terminal": "t9"}}]}});
    let space =
        json!({"id": "spnew", "name": "", "folder": "/Users/owner/src/beta", "tabs": [tab]});
    let original = fixture!("unknown-fields");
    let next = changed(apply(&original, &new_space("t9"))?)?;
    assert_eq!(next["spaces"][1], space);
    assert_eq!(paths(&original, &next), vec!["spaces[1]"]);
    for empty in [
        Value::Null,
        json!({}),
        json!({"spaces": null}),
        json!({"x": 1}),
    ] {
        let next = changed(apply(&empty, &new_space("t9"))?)?;
        assert_eq!(next["spaces"], json!([space]), "{empty}");
        check_write(&empty, &next, &new_space("t9")).map_err(|e| format!("{empty}: {e}"))?;
    }
    Ok(())
}

#[test]
fn split_in_the_same_direction_adds_a_sibling_and_halves_the_weight() -> Outcome {
    let original = fixture!("unknown-fields");
    let next = changed(apply(&original, &split("t1", Axis::Row))?)?;
    let children = &next["spaces"][0]["tabs"][0]["root"]["children"];
    assert_eq!(
        *children,
        json!([
            {"id": P1, "type": "block", "size": {"weight": 0.5}, "x-node": "block",
                "content": {"terminal": "t1"}},
            {"id": "pnew", "type": "block", "content": {"terminal": "t9"}, "size": {"weight": 0.5}}
        ])
    );
    // A block without a size: neither block gets one (owner answer Q8).
    let plain = fixture!("core-roundtrip-written");
    let next = changed(apply(&plain, &split("t1", Axis::Row))?)?;
    let children = &next["spaces"][0]["tabs"][0]["root"]["children"];
    assert_eq!(
        children[1],
        json!({"id": "pnew", "type": "block", "content": {"terminal": "t9"}})
    );
    assert!(children[0].get("size").is_none());
    Ok(())
}

#[test]
fn split_in_the_other_direction_wraps_the_block() -> Outcome {
    let original = fixture!("unknown-fields");
    let next = changed(apply(&original, &split("t1", Axis::Column))?)?;
    let root = &next["spaces"][0]["tabs"][0]["root"];
    assert_eq!(
        root["children"],
        json!([{"id": "snew", "type": "column", "size": {"weight": 1.0}, "children": [
            {"id": P1, "type": "block", "x-node": "block", "content": {"terminal": "t1"}},
            {"id": "pnew", "type": "block", "content": {"terminal": "t9"}}
        ]}])
    );
    assert_eq!(root["id"], json!(TB1));
    check_write(&original, &next, &split("t1", Axis::Column))?;
    // A block one level down goes beside itself in its own column.
    let nested = json!({"spaces": [{"id": "spa", "tabs": [{"id": "tba", "root": {"id": "tba",
        "type": "row", "children": [{"id": "sa", "type": "column", "children": [
            {"id": "pa", "type": "block", "content": {"terminal": "t1"}}]}]}}]}]});
    let next = changed(apply(&nested, &split("t1", Axis::Column))?)?;
    let inner = &next["spaces"][0]["tabs"][0]["root"]["children"][0]["children"];
    assert_eq!(inner.as_array().map(Vec::len), Some(2));
    check_write(&nested, &next, &split("t1", Axis::Column))?;
    Ok(())
}

#[test]
fn split_only_writes_a_one_pane_tree() -> Outcome {
    for (original, terminal) in [
        (fixture!("tree-px"), "t1"),
        (fixture!("tree-three-panes"), "t1"),
        (fixture!("tree-float"), "t1"),
        (fixture!("tree-mod"), "t1"),
    ] {
        assert!(
            matches!(
                apply(&original, &split(terminal, Axis::Row)),
                Err(DocError::Undrawable)
            ),
            "{original}"
        );
    }
    let root_block = json!({"spaces": [{"tabs": [{"id": "tba", "root": {"id": "tba",
        "type": "block", "content": {"terminal": "t1"}}}]}]});
    assert!(matches!(
        apply(&root_block, &split("t1", Axis::Row)),
        Err(DocError::Undrawable)
    ));
    Ok(())
}

#[test]
fn intents_are_idempotent() -> Outcome {
    let original = fixture!("unknown-fields");
    for intent in [
        rename_space(SP1, "Alpha"),
        rename_tab(TB1, "Build"),
        new_tab(SP1, "t9"),
        new_space("t9"),
        split("t1", Axis::Row),
        split("t1", Axis::Column),
    ] {
        let once = changed(apply(&original, &intent)?)?;
        assert_eq!(apply(&once, &intent)?, Applied::Unchanged, "{intent:?}");
    }
    Ok(())
}

#[test]
fn new_tab_twice_same_document() -> Outcome {
    let original = fixture!("unknown-fields");
    let once = changed(apply(&original, &new_tab(SP1, "t9"))?)?;
    let again = Intent::NewTab {
        space: SP1.into(),
        tab_id: "tbother".into(),
        pane_id: "pother".into(),
        terminal: "t9".into(),
    };
    assert_eq!(apply(&once, &again)?, Applied::Unchanged);
    assert_eq!(apply(&once, &new_space("t9"))?, Applied::Unchanged);
    // A terminal the official app already took in (here a floating one).
    assert_eq!(apply(&original, &new_tab(SP1, "t3"))?, Applied::Unchanged);
    Ok(())
}

#[test]
fn new_ids_are_the_official_format() -> Outcome {
    for prefix in ["sp", "tb", "p", "s"] {
        let (a, b) = (new_id(prefix)?, new_id(prefix)?);
        assert_ne!(a, b);
        let hex = a.strip_prefix(prefix).ok_or("no prefix")?;
        assert_eq!(hex.len(), 32, "{a}");
        assert!(
            hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "{a}"
        );
    }
    Ok(())
}

/// `original` with a filler string in its space, `extra` bytes larger.
fn padded(original: &Value, extra: usize) -> Result<Value, Box<dyn std::error::Error>> {
    let mut value = original.clone();
    value["spaces"][0]["x-space"] = json!("");
    let base = serde_json::to_vec(&value)?.len();
    value["spaces"][0]["x-space"] = json!("x".repeat(extra.saturating_sub(base)));
    Ok(value)
}

#[test]
fn check_rejects_too_large() -> Outcome {
    let original = fixture!("unknown-fields");
    let limit = padded(&original, MAX_BYTES)?;
    assert_eq!(serde_json::to_vec(&limit)?.len(), MAX_BYTES);
    check_write(&original, &limit, &Intent::Replace(limit.clone()))?;
    let over = padded(&original, MAX_BYTES + 1)?;
    assert_eq!(
        check_write(&original, &over, &Intent::Replace(over.clone())),
        Err(WriteCheck::TooLarge)
    );
    Ok(())
}

#[test]
fn check_rejects_unreadable() -> Outcome {
    let original = fixture!("unknown-fields");
    let candidate = json!({"spaces": {}});
    assert_eq!(
        check_write(&original, &candidate, &Intent::Replace(candidate.clone())),
        Err(WriteCheck::Unreadable)
    );
    Ok(())
}

#[test]
fn check_rejects_outside_path() -> Outcome {
    let original = fixture!("unknown-fields");
    let touch = Intent::RenameSpaceAlsoTouch {
        id: SP1.into(),
        name: "Alpha".into(),
    };
    let candidate = changed(apply(&original, &touch)?)?;
    assert_eq!(
        check_write(&original, &candidate, &touch),
        Err(WriteCheck::OutsidePath)
    );
    // A real intent's candidate with another change, or keys reordered.
    let intent = rename_space(SP1, "Alpha");
    let mut extra = changed(apply(&original, &intent)?)?;
    extra["x-top"] = json!(null);
    let mut reordered = changed(apply(&original, &intent)?)?;
    if let Some(space) = reordered["spaces"][0].as_object_mut() {
        let tabs = space.shift_remove("tabs").ok_or("no tabs")?;
        space.shift_insert(0, "tabs".into(), tabs);
    }
    for candidate in [extra, reordered] {
        assert_eq!(
            check_write(&original, &candidate, &intent),
            Err(WriteCheck::OutsidePath)
        );
    }
    let tab = new_tab(SP1, "t9");
    let mut two = changed(apply(&original, &tab)?)?;
    two["spaces"][0]["tabs"][1]["name"] = json!("x");
    assert_eq!(
        check_write(&original, &two, &tab),
        Err(WriteCheck::OutsidePath)
    );
    Ok(())
}

#[test]
fn check_rejects_root_id() -> Outcome {
    let original = fixture!("unknown-fields");
    let mut candidate = original.clone();
    candidate["spaces"][0]["tabs"][0]["root"]["id"] = json!("tbother");
    assert_eq!(
        check_write(&original, &candidate, &Intent::Replace(candidate.clone())),
        Err(WriteCheck::RootId)
    );
    Ok(())
}

#[test]
fn check_rejects_duplicate_pane() -> Outcome {
    let original = fixture!("unknown-fields");
    let mut candidate = original.clone();
    let float = candidate["spaces"][0]["tabs"][1]["root"]["floating"][0].clone();
    let mut twin = float.clone();
    twin["content"] = json!({"terminal": "t8"});
    candidate["spaces"][0]["tabs"][1]["root"]["floating"] = json!([float, twin]);
    assert_eq!(
        check_write(&original, &candidate, &Intent::Replace(candidate.clone())),
        Err(WriteCheck::DuplicatePane)
    );
    // The same pane id in two tabs is not a duplicate.
    let mut across = original.clone();
    across["spaces"][0]["tabs"][1]["root"]["children"][0]["id"] = json!(P1);
    check_write(&original, &across, &Intent::Replace(across.clone()))?;
    Ok(())
}

#[test]
fn every_intent_passes_the_check_on_every_fixture() -> Outcome {
    let mut written = 0;
    for (name, original) in readable()? {
        let view = parse_view(&original)?;
        let mut intents = vec![new_space("t9")];
        for space in view.spaces.iter().filter(|space| !space.id.is_empty()) {
            intents.push(new_tab(&space.id, "t9"));
            for tab in &space.tabs {
                if tab.drawable == Drawable::One {
                    intents.push(split(&tab.terminals[0], Axis::Row));
                    intents.push(split(&tab.terminals[0], Axis::Column));
                }
            }
        }
        for intent in intents {
            let next = changed(apply(&original, &intent)?)?;
            check_write(&original, &next, &intent)
                .map_err(|e| format!("{name} {intent:?}: {e}"))?;
            parse_view(&next)?;
            written += 1;
        }
    }
    assert!(written >= 20, "only {written} intents were tried");
    Ok(())
}

// The mock core: herdr-core's document semantics through the real client.

fn connect(host: &Host) -> Result<sheprd_core_client::CoreBackend, Box<dyn std::error::Error>> {
    Ok(host.recorded(false)?.0)
}

fn refused_kind(error: CoreError) -> Option<String> {
    match error {
        CoreError::Refused { kind, .. } => kind,
        _ => None,
    }
}

/// Notices until one matches, or none within the bound.
fn notice(
    subscription: &mut sheprd_core_client::Subscription,
    within: Duration,
    want: impl Fn(&Notice) -> bool,
) -> Result<Option<Notice>, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(found) = subscription
            .next(Duration::from_millis(50))?
            .into_iter()
            .find(|n| want(n))
        {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

#[test]
fn mock_unwritten_document_is_null_at_version_zero() -> Outcome {
    let host = Host::default();
    let doc = connect(&host)?.core().doc_get(NAME)?;
    assert_eq!((doc.value, doc.version), (Value::Null, 0));
    Ok(())
}

#[test]
fn mock_doc_set_checks_the_version() -> Outcome {
    let host = Host::default();
    let backend = connect(&host)?;
    let first = json!({"spaces": []});
    assert_eq!(backend.core().doc_set(NAME, &first, 0)?, 1);
    let stale = backend.core().doc_set(NAME, &json!({"x": 1}), 0);
    assert_eq!(
        stale.map_err(refused_kind).err(),
        Some(Some("conflict".into()))
    );
    assert_eq!(host.doc().doc(), (first.clone(), 1));
    assert_eq!(host.doc().sets(), vec![(first, 0), (json!({"x": 1}), 0)]);
    Ok(())
}

#[test]
fn mock_refuses_more_than_one_mib() -> Outcome {
    let host = Host::default();
    let backend = connect(&host)?;
    let large = json!({"x": "x".repeat(MAX_BYTES)});
    let refused = backend.core().doc_set(NAME, &large, 0);
    assert_eq!(
        refused.map_err(refused_kind).err(),
        Some(Some("invalid_params".into()))
    );
    assert_eq!(host.doc().doc(), (Value::Null, 0));
    Ok(())
}

#[test]
fn mock_round_trip_is_byte_identical_for_every_fixture() -> Outcome {
    for (name, original) in readable()? {
        let host = Host::default();
        let version = host.doc().set_silent(original.clone());
        let backend = connect(&host)?;
        let read = backend.core().doc_get(NAME)?;
        assert_eq!(read.version, version, "{name}");
        backend.core().doc_set(NAME, &read.value, read.version)?;
        let (stored, _) = host.doc().doc();
        assert_eq!(
            serde_json::to_vec(&stored)?,
            serde_json::to_vec(&original)?,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn mock_events_follow_writes() -> Outcome {
    let host = Host::default();
    // Dropping a connection ends its streams, so the reader is kept.
    let reader = connect(&host)?;
    let mut events = reader.core().subscribe()?;
    let writer = connect(&host)?;
    let bound = Duration::from_secs(5);
    let doc = |version: u64| {
        move |n: &Notice| {
            matches!(n, Notice::Doc { name, version: v, machine: None, .. }
                if name == NAME && *v == version)
        }
    };
    writer.core().doc_set(NAME, &json!({"spaces": []}), 0)?;
    assert!(notice(&mut events, bound, doc(1))?.is_some());
    host.doc().set_external(json!({"spaces": [{"id": "spx"}]}));
    assert!(notice(&mut events, bound, doc(2))?.is_some());
    // A write whose event was lost is heard of by no notice at all.
    host.doc().set_silent(json!({}));
    let quiet = notice(&mut events, Duration::from_millis(300), |n| {
        matches!(n, Notice::Doc { .. })
    })?;
    assert_eq!(quiet, None);
    host.doc().missed();
    let missed = notice(&mut events, bound, |n| matches!(n, Notice::Missed { .. }))?;
    assert!(missed.is_some());
    assert_eq!(writer.core().doc_get(NAME)?.version, 3);
    Ok(())
}

#[test]
fn mock_conflict_next_is_another_app_writing_first() -> Outcome {
    let host = Host::default();
    host.doc().set_silent(json!({"spaces": []}));
    host.doc().conflict_next(1);
    let backend = connect(&host)?;
    let mine = json!({"spaces": [{"id": "spa"}]});
    let refused = backend.core().doc_set(NAME, &mine, 1);
    assert_eq!(
        refused.map_err(refused_kind).err(),
        Some(Some("conflict".into()))
    );
    let latest = backend.core().doc_get(NAME)?;
    assert_eq!(
        (latest.value.clone(), latest.version),
        (json!({"spaces": [], "external": 1}), 2)
    );
    assert_eq!(backend.core().doc_set(NAME, &mine, latest.version)?, 3);
    assert_eq!(host.doc().sets().len(), 2);
    Ok(())
}

#[test]
fn mock_run_records_labels() -> Outcome {
    let host = Host::default();
    let backend = connect(&host)?;
    let labels = BTreeMap::from([
        ("herdr.space", "spa"),
        ("herdr.tab", "tba"),
        ("herdr.seen", "0"),
    ]);
    let terminal = backend.core().run(&Run {
        keep: true,
        labels: labels.clone(),
        ..Run::default()
    })?;
    let want: BTreeMap<String, String> = labels
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    assert_eq!(terminal.labels, want);
    let listed = backend.core().list(true)?;
    let found = listed
        .terminals
        .iter()
        .find(|t| t.address == terminal.address)
        .ok_or("not listed")?;
    assert_eq!(found.labels, want);
    Ok(())
}
