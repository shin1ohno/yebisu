//! `Intent::PruneSpace`: it takes ended panes, the tabs that leaves empty
//! and the space when no tab is left out of the latest document, keeps
//! every other byte (unknown fields too), leaves a space changed since it
//! was read, and the pre-write gate refuses a candidate that changed
//! anything else, inside the space or outside it.
use super::herdr_doc::*;
use serde_json::{Value, json};
use std::path::Path;

type Outcome = Result<(), Box<dyn std::error::Error>>;

fn readable() -> Result<Vec<(String, Value)>, Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/herdr-app");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        if path.extension().is_some_and(|ext| ext == "json") && !name.starts_with("unreadable-") {
            found.push((
                name,
                serde_json::from_str(&std::fs::read_to_string(&path)?)?,
            ));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

/// Space `i` with `ended` terminals, or every terminal it names.
fn prune(value: &Value, i: usize, ended: Option<&[&str]>) -> Option<Intent> {
    let space = value.get("spaces")?.get(i)?.clone();
    let id = space.get("id")?.as_str()?.to_owned();
    let ended = match ended {
        Some(ended) => ended.iter().map(|t| (*t).to_owned()).collect(),
        None => parse_view(value)
            .ok()?
            .spaces
            .get(i)?
            .tabs
            .iter()
            .flat_map(|tab| tab.terminals.clone())
            .collect(),
    };
    Some(Intent::PruneSpace { id, space, ended })
}

fn remove(value: &Value, i: usize) -> Option<Intent> {
    prune(value, i, None)
}

#[test]
fn prune_changes_only_that_space_on_every_fixture() -> Outcome {
    let (mut removed, mut pruned) = (0, 0);
    for (name, original) in readable()? {
        let count = original
            .get("spaces")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        for i in 0..count {
            let Some(intent) = remove(&original, i) else {
                continue;
            };
            let next = match apply(&original, &intent)? {
                Applied::Changed(next) => next,
                // Only tabs that are not plain: nothing to take out.
                Applied::Unchanged => continue,
            };
            check_write(&original, &next, &intent).map_err(|e| format!("{name}: {e}"))?;
            parse_view(&next)?;
            let before = original["spaces"].as_array().ok_or("spaces")?;
            let after = next["spaces"].as_array().ok_or("spaces")?;
            let mut others = before.clone();
            others.remove(i);
            if after.len() == before.len() {
                pruned += 1;
                let mut kept = after.clone();
                kept.remove(i);
                assert_eq!(kept, others, "{name}: another space changed");
            } else {
                removed += 1;
                assert_eq!(*after, others, "{name}");
            }
            // Byte for byte outside `spaces`: key order and unknown fields.
            let mut outside = next.clone();
            outside["spaces"] = original["spaces"].clone();
            assert_eq!(
                serde_json::to_string(&outside)?,
                serde_json::to_string(&original)?,
                "{name}"
            );
        }
    }
    assert!(removed >= 3, "only {removed} removals were tried");
    assert!(pruned + removed >= 5, "only {pruned} prunes were tried");
    Ok(())
}

#[test]
fn space_with_a_live_terminal_loses_only_its_ended_panes() -> Outcome {
    let original = json!({"spaces": [{"id": "sp1", "name": "env", "tabs": [
        {"id": "tb1", "name": "", "root": {"type": "row", "id": "tb1", "children": [
            {"type": "block", "id": "p1", "content": {"terminal": "t1"}},
            {"type": "block", "id": "p2", "content": {"terminal": "t2"}}]}},
        {"id": "tb2", "name": "", "root": {"type": "row", "id": "tb2", "children": [
            {"type": "block", "id": "p3", "content": {"terminal": "t3"}}]}}]}]});
    let intent = prune(&original, 0, Some(&["t1", "t3"])).ok_or("no space")?;
    let Applied::Changed(next) = apply(&original, &intent)? else {
        return Err("nothing pruned".into());
    };
    check_write(&original, &next, &intent)?;
    assert_eq!(
        next["spaces"][0]["tabs"],
        json!([{"id": "tb1", "name": "", "root": {"type": "row", "id": "tb1", "children": [
            {"type": "block", "id": "p2", "content": {"terminal": "t2"},
                "size": {"weight": 2.0}}]}}])
    );
    // The gate holds the space to exactly that prune: one pane more out
    // (the live t2's) is refused.
    let mut more = next.clone();
    more["spaces"][0]["tabs"][0]["root"]["children"] = json!([]);
    assert_eq!(
        check_write(&original, &more, &intent),
        Err(WriteCheck::OutsidePath)
    );
    Ok(())
}

fn three() -> Value {
    json!({"spaces": [
        {"id": "sp1", "name": "", "folder": "/a", "tabs": [], "newer": 1},
        {"id": "sp2", "name": "", "folder": "/b", "tabs": [
            {"id": "tb2", "name": "", "root": {"type": "block", "id": "tb2",
                "content": {"terminal": "t2"}}}]},
        {"id": "sp3", "name": "x", "folder": "/c", "tabs": []}
    ], "future": {"kept": true}})
}

#[test]
fn removed_space_is_unchanged_the_second_time() -> Outcome {
    let original = three();
    let intent = remove(&original, 1).ok_or("no space")?;
    let Applied::Changed(next) = apply(&original, &intent)? else {
        return Err("not removed".into());
    };
    assert_eq!(apply(&next, &intent)?, Applied::Unchanged);
    assert_eq!(next["future"], json!({"kept": true}));
    assert_eq!(next["spaces"][0]["newer"], json!(1));
    Ok(())
}

#[test]
fn space_changed_since_it_was_read_stays() -> Outcome {
    let original = three();
    let intent = remove(&original, 1).ok_or("no space")?;
    // Another app added a tab (a new terminal) after the read.
    let mut latest = original.clone();
    latest["spaces"][1]["tabs"]
        .as_array_mut()
        .ok_or("tabs")?
        .push(
            json!({"id": "tb9", "name": "", "root": {"type": "block", "id": "tb9",
            "content": {"terminal": "t9"}}}),
        );
    assert_eq!(apply(&latest, &intent)?, Applied::Unchanged);
    // Renamed after the read.
    let mut renamed = original.clone();
    renamed["spaces"][1]["name"] = json!("Build");
    assert_eq!(apply(&renamed, &intent)?, Applied::Unchanged);
    // Moved to another place in the order: still the same space.
    let mut moved = original.clone();
    if let Some(Value::Array(spaces)) = moved.get_mut("spaces") {
        let space = spaces.remove(1);
        spaces.insert(0, space);
    }
    let Applied::Changed(next) = apply(&moved, &intent)? else {
        return Err("a moved space is still the one read".into());
    };
    check_write(&moved, &next, &intent)?;
    Ok(())
}

#[test]
fn gate_refuses_a_removal_that_changes_anything_else() -> Outcome {
    let original = three();
    let intent = remove(&original, 1).ok_or("no space")?;
    let Applied::Changed(mut next) = apply(&original, &intent)? else {
        return Err("not removed".into());
    };
    check_write(&original, &next, &intent)?;
    // Two spaces gone.
    let mut both = next.clone();
    if let Some(Value::Array(spaces)) = both.get_mut("spaces") {
        spaces.remove(0);
    }
    assert_eq!(
        check_write(&original, &both, &intent),
        Err(WriteCheck::OutsidePath)
    );
    // Another field touched.
    next["future"] = json!(null);
    assert_eq!(
        check_write(&original, &next, &intent),
        Err(WriteCheck::OutsidePath)
    );
    Ok(())
}

#[test]
fn unknown_space_or_document_is_unchanged() -> Outcome {
    let intent = Intent::PruneSpace {
        id: "spX".into(),
        space: json!({"id": "spX"}),
        ended: vec!["t1".into()],
    };
    assert_eq!(apply(&three(), &intent)?, Applied::Unchanged);
    assert_eq!(apply(&Value::Null, &intent)?, Applied::Unchanged);
    Ok(())
}
