//! `Intent::RemoveSpace` (Close Space, #231): the space goes with all its
//! tabs, every other space and every byte outside `spaces` stay as they
//! were, and the pre-write gate refuses a candidate that changed anything
//! else.
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

fn close(id: &str) -> Intent {
    Intent::RemoveSpace { id: id.to_owned() }
}

#[test]
fn close_takes_only_that_space_on_every_fixture() -> Outcome {
    let mut closed = 0;
    for (name, original) in readable()? {
        let view = parse_view(&original)?;
        for (i, space) in view.spaces.iter().enumerate() {
            if space.id.is_empty() {
                continue;
            }
            let intent = close(&space.id);
            let Applied::Changed(next) = apply(&original, &intent)? else {
                return Err(format!("{name}: space {i} was not closed").into());
            };
            check_write(&original, &next, &intent).map_err(|e| format!("{name}: {e}"))?;
            parse_view(&next)?;
            let mut others = original["spaces"].as_array().ok_or("spaces")?.clone();
            others.remove(i);
            // The other spaces and their tabs, byte for byte.
            assert_eq!(
                serde_json::to_string(&next["spaces"])?,
                serde_json::to_string(&others)?,
                "{name}: another space changed"
            );
            let mut outside = next.clone();
            outside["spaces"] = original["spaces"].clone();
            assert_eq!(
                serde_json::to_string(&outside)?,
                serde_json::to_string(&original)?,
                "{name}: outside the spaces"
            );
            closed += 1;
        }
    }
    assert!(closed >= 5, "only {closed} closes were tried");
    Ok(())
}

#[test]
fn a_space_already_gone_changes_nothing() -> Outcome {
    let original = json!({"spaces": [{"id": "sp1", "name": "", "folder": "/x", "tabs": []}]});
    assert_eq!(apply(&original, &close("sp2"))?, Applied::Unchanged);
    Ok(())
}

#[test]
fn the_gate_refuses_a_close_that_touched_anything_else() -> Outcome {
    let original = json!({"spaces": [
        {"id": "sp1", "name": "a", "folder": "/a", "tabs": []},
        {"id": "sp2", "name": "b", "folder": "/b", "tabs": []}],
        "external": 1});
    let intent = close("sp1");
    // The right candidate passes.
    check_write(
        &original,
        &json!({"spaces": [{"id": "sp2", "name": "b", "folder": "/b", "tabs": []}],
            "external": 1}),
        &intent,
    )?;
    for (what, candidate) in [
        (
            "renamed the other space",
            json!({"spaces": [{"id": "sp2", "name": "c", "folder": "/b", "tabs": []}],
                "external": 1}),
        ),
        (
            "closed the other space",
            json!({"spaces": [{"id": "sp1", "name": "a", "folder": "/a", "tabs": []}],
                "external": 1}),
        ),
        ("closed both", json!({"spaces": [], "external": 1})),
        (
            "changed a field outside",
            json!({"spaces": [{"id": "sp2", "name": "b", "folder": "/b", "tabs": []}],
                "external": 2}),
        ),
    ] {
        assert_eq!(
            check_write(&original, &candidate, &intent),
            Err(WriteCheck::OutsidePath),
            "{what}"
        );
    }
    Ok(())
}
