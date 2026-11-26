//! Ended panes out of a `herdr-app` space, as the official app takes them
//! out (`herdr-app/src/model.rs` `remove_pane`, `remove_tab`,
//! `remove_space`; `src/layout.rs` `Node::remove`, `remove_inner`,
//! `collapse`), on the document's `Value` so every other key stays.
//!
//! One pane at a time, in depth-first order (the order `reconcile` lists
//! them in): the pane leaves its row or column and its share (a `weight`,
//! 1 when it has no size) goes to the neighbour before it, else the next,
//! unless either is sized in `px`; then a row or column left empty vanishes
//! and one left with one child gives way to it, which takes its size. The
//! tab's root keeps its id and is never replaced. A tab with no pane left
//! goes, and the caller removes a space with no tab left.
//!
//! Only a plain tab is changed: blocks of terminals in rows and columns. A
//! tab with a stack, a floating block, a mod or an agent is kept whole.
use serde_json::{Map, Number, Value, json};
use std::collections::BTreeSet;

/// Whether `node` holds only terminal blocks in rows and columns.
pub(crate) fn plain(node: &Value) -> bool {
    let Some(object) = node.as_object() else {
        return false;
    };
    if object.get("floating").is_some_and(|f| !f.is_null()) {
        return false;
    }
    match object.get("type").and_then(Value::as_str) {
        Some("block") => block_terminal(node).is_some(),
        Some("row" | "column") => children(node).iter().all(plain),
        _ => false,
    }
}

/// `space` without the panes whose terminal is in `ended`, and without the
/// tabs that leaves empty. Tabs that are not plain stay as they are.
pub(crate) fn prune_space(space: &Value, ended: &BTreeSet<String>) -> Value {
    let mut next = space.clone();
    let Some(Value::Array(tabs)) = next.get_mut("tabs") else {
        return next;
    };
    tabs.retain_mut(|tab| {
        let Some(root) = tab.get_mut("root") else {
            // A tab without a tree has no pane: the official app has none.
            return false;
        };
        if !plain(root) {
            return true;
        }
        if block_terminal(root).is_some_and(|terminal| ended.contains(terminal)) {
            return false;
        }
        for pane in ended_panes(root, ended) {
            remove_inner(root, &pane);
            collapse(root);
        }
        has_block(root)
    });
    next
}

/// The ids of the blocks under `node` whose terminal ended, depth first.
fn ended_panes(node: &Value, ended: &BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();
    for child in children(node) {
        if let Some(terminal) = block_terminal(child) {
            if ended.contains(terminal)
                && let Some(id) = child.get("id").and_then(Value::as_str)
            {
                found.push(id.to_owned());
            }
        } else {
            found.extend(ended_panes(child, ended));
        }
    }
    found
}

/// `remove_inner`: takes the child `id` out of the node that holds it.
fn remove_inner(node: &mut Value, id: &str) -> bool {
    let container = matches!(kind(node), Some("row" | "column"));
    let Some(Value::Array(items)) = node.get_mut("children") else {
        return false;
    };
    if let Some(i) = items
        .iter()
        .position(|child| child.get("id").and_then(Value::as_str) == Some(id))
    {
        let removed = items.remove(i);
        if container
            && let Some(gone) = weight(removed.get("size"))
            && let Some(neighbour) = items.get_mut(i.saturating_sub(1))
            && let Some(own) = weight(neighbour.get("size"))
        {
            set_weight(neighbour, own + gone);
        }
        return true;
    }
    items.iter_mut().any(|child| remove_inner(child, id))
}

/// `collapse`, below `node`: empty rows and columns vanish, and one with a
/// single child gives way to it.
fn collapse(node: &mut Value) {
    let Some(Value::Array(items)) = node.get_mut("children") else {
        return;
    };
    for child in items.iter_mut() {
        collapse(child);
    }
    items.retain(|child| !(container(child) && children(child).is_empty()));
    for child in items.iter_mut() {
        if container(child) && children(child).len() == 1 {
            let size = child.get("size").cloned();
            let Some(Value::Array(inner)) = child.get_mut("children") else {
                continue;
            };
            let mut only = inner.remove(0);
            if let Some(object) = only.as_object_mut() {
                match size {
                    Some(size) => {
                        object.insert("size".into(), size);
                    }
                    None => {
                        object.shift_remove("size");
                    }
                }
            }
            *child = only;
        }
    }
}

/// A share as the official app counts it: no size is 1, `px` is none.
fn weight(size: Option<&Value>) -> Option<f64> {
    match size {
        None | Some(Value::Null) => Some(1.0),
        Some(Value::Object(size)) => match (size.get("weight"), size.get("px")) {
            (Some(weight), _) => weight.as_f64(),
            (None, Some(_)) => None,
            (None, None) => Some(1.0),
        },
        Some(_) => None,
    }
}

/// Sets the node's `weight`, keeping any other key of its size.
fn set_weight(node: &mut Value, weight: f64) {
    let Some(number) = Number::from_f64(weight) else {
        return;
    };
    let Some(object) = node.as_object_mut() else {
        return;
    };
    match object.get_mut("size") {
        Some(Value::Object(size)) => {
            size.insert("weight".into(), Value::Number(number));
        }
        _ => {
            let mut size = Map::new();
            size.insert("weight".into(), Value::Number(number));
            object.insert("size".into(), json!(size));
        }
    }
}

fn has_block(node: &Value) -> bool {
    block_terminal(node).is_some() || children(node).iter().any(has_block)
}

fn kind(node: &Value) -> Option<&str> {
    node.get("type").and_then(Value::as_str)
}

fn container(node: &Value) -> bool {
    matches!(kind(node), Some("row" | "column"))
}

fn children(node: &Value) -> &[Value] {
    node.get("children")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn block_terminal(node: &Value) -> Option<&str> {
    if kind(node) != Some("block") {
        return None;
    }
    node.get("content")?.get("terminal")?.as_str()
}
