//! The official app's `herdr-app` document (issue #144), read and changed as
//! a `serde_json::Value`.
//!
//! The document lives in the `default` session's herdr-core and holds the
//! official app's spaces, tabs, names and pane trees. The official app drops
//! unknown fields at the top level and in tree nodes, so a newer app's fields
//! survive only when yebisu never rebuilds the document from a typed copy:
//! the `View` is for drawing, and every write applies one `Intent` to the
//! latest `Value`, changing only the keys the intent names (Design C, D-1).
//! `check_write` is the last gate before `doc.set` (Design D-4): a value the
//! official app cannot read puts its layout out of use on the Mac (R1).
//! Everything here is pure, apart from `new_id`'s random bytes.
use serde_json::{Map, Value, json};
use std::{collections::BTreeSet, io::Read};

/// The document's name in herdr-core.
pub(crate) const NAME: &str = "herdr-app";
/// herdr-core refuses larger values (`invalid_params`).
pub(crate) const MAX_BYTES: usize = 1_048_576;

#[derive(Debug, thiserror::Error)]
pub(crate) enum DocError {
    // keep-sorted start
    #[error("the herdr-app document has no {0} with that id")]
    Missing(&'static str),
    #[error("no random bytes for a new herdr-app id")]
    Random(#[source] std::io::Error),
    #[error("the herdr-app layout of this tab cannot be split here")]
    Undrawable,
    #[error("the herdr-app document is unreadable at {0}")]
    Unreadable(String),
    // keep-sorted end
}

/// The pre-write gate a candidate failed (Design D-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum WriteCheck {
    // keep-sorted start
    #[error("duplicate-pane")]
    DuplicatePane,
    #[error("outside-path")]
    OutsidePath,
    #[error("root-id")]
    RootId,
    #[error("too-large")]
    TooLarge,
    #[error("unreadable")]
    Unreadable,
    // keep-sorted end
}

/// What yebisu draws from the document. Never serialised back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct View {
    pub(crate) spaces: Vec<SpaceView>,
}

/// Absent strings read as empty: every field of the document is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SpaceView {
    pub(crate) id: String,
    /// Empty is the automatic name.
    pub(crate) name: String,
    pub(crate) folder: String,
    pub(crate) tabs: Vec<TabView>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TabView {
    pub(crate) id: String,
    /// Empty is the automatic name (the tab's position).
    pub(crate) name: String,
    /// Terminal addresses in depth-first order: `children` order, then
    /// `floating`.
    pub(crate) terminals: Vec<String>,
    pub(crate) drawable: Drawable,
    /// The tree holds something other than terminal blocks and their
    /// rows and columns: a mod, an agent, a stack, a floating block.
    pub(crate) other: bool,
}

/// Whether yebisu's at most two panes in one direction can show the tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Drawable {
    One,
    /// `vertical` is a `column`: yebisu's SplitDown.
    Two {
        vertical: bool,
    },
    /// Anything else; shown as its first two terminals (Design F).
    #[default]
    Not,
}

/// The direction of a split: `row` is SplitRight, `column` SplitDown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    Row,
    Column,
}

impl Axis {
    fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
        }
    }
}

/// One change, applied to the latest document (Design C).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Intent {
    RenameSpace {
        id: String,
        name: String,
    },
    RenameTab {
        id: String,
        name: String,
    },
    NewTab {
        space: String,
        tab_id: String,
        pane_id: String,
        terminal: String,
    },
    NewSpace {
        space_id: String,
        folder: String,
        tab_id: String,
        pane_id: String,
        terminal: String,
    },
    Split {
        terminal: String,
        axis: Axis,
        new_pane: String,
        new_container: String,
        new_terminal: String,
    },
    /// The panes of space `id` whose terminal is in `ended` go, then the
    /// tabs and the space that leaves empty, as the official app removes
    /// them (`herdr_doc_prune`). `space` is the space as it was read: a
    /// space changed since then stays as it is.
    PruneSpace {
        id: String,
        space: Value,
        ended: Vec<String>,
    },
    /// Close Space (#231): space `id` goes with all its tabs, as the
    /// official app's `space.close` removes it (`model.rs` `remove_space`).
    /// Its terminals are ended by the caller.
    RemoveSpace {
        id: String,
    },
    /// The iPad's saved `default` tabs, moved once into one new space
    /// named "iPad" (Design G, owner decision 4).
    Migrate {
        space_id: String,
        tabs: Vec<MigratedTab>,
    },
    /// Tests: write this value as it is, so each gate is tested alone.
    #[cfg(test)]
    Replace(Value),
    /// Tests: a rename that also renames the space's first tab.
    #[cfg(test)]
    RenameSpaceAlsoTouch {
        id: String,
        name: String,
    },
}

/// One saved tab for `Intent::Migrate`: its new id, each pane's new id and
/// terminal, and whether its two panes are stacked.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MigratedTab {
    pub(crate) tab_id: String,
    pub(crate) panes: Vec<(String, String)>,
    pub(crate) vertical: bool,
}

/// The name of the space the iPad's saved tabs move into (decision 4).
pub(crate) const MIGRATED_SPACE: &str = "iPad";

/// `Unchanged` is the idempotent case: nothing to write.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Applied {
    Unchanged,
    Changed(Value),
}

/// The document's shape check (Design B-4): `Err` is the official app's
/// "unreadable" state, in which yebisu writes nothing.
pub(crate) fn parse_view(value: &Value) -> Result<View, DocError> {
    let top = match value {
        Value::Null => return Ok(View::default()),
        Value::Object(top) => top,
        _ => return Err(unreadable("")),
    };
    let mut spaces = Vec::new();
    for (i, space) in array(top.get("spaces"), "spaces")?.iter().enumerate() {
        let at = format!("spaces[{i}]");
        let space = space.as_object().ok_or_else(|| unreadable(&at))?;
        let mut tabs = Vec::new();
        for (j, tab) in array(space.get("tabs"), &format!("{at}.tabs"))?
            .iter()
            .enumerate()
        {
            let at = format!("{at}.tabs[{j}]");
            let tab = tab.as_object().ok_or_else(|| unreadable(&at))?;
            let mut walk = Walk::default();
            if let Some(root) = present(tab.get("root")) {
                walk_node(root, &format!("{at}.root"), &mut walk)?;
            }
            let drawable = match (walk.other, walk.terminals.len()) {
                (false, 1) => Drawable::One,
                (false, 2) => tab
                    .get("root")
                    .and_then(pair)
                    .map_or(Drawable::Not, |kind| Drawable::Two {
                        vertical: kind == "column",
                    }),
                _ => Drawable::Not,
            };
            tabs.push(TabView {
                id: text(tab, "id", &at)?,
                name: text(tab, "name", &at)?,
                terminals: walk.terminals,
                drawable,
                other: walk.other,
            });
        }
        spaces.push(SpaceView {
            id: text(space, "id", &at)?,
            name: text(space, "name", &at)?,
            folder: text(space, "folder", &at)?,
            tabs,
        });
    }
    Ok(View { spaces })
}

/// What a tree holds, collected in depth-first order.
#[derive(Default)]
struct Walk {
    terminals: Vec<String>,
    /// A stack, floating block, non-terminal or unknown block or type.
    other: bool,
}

fn walk_node(value: &Value, at: &str, walk: &mut Walk) -> Result<(), DocError> {
    let object = value.as_object().ok_or_else(|| unreadable(at))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| unreadable(&format!("{at}.type")))?;
    if present(object.get("id")).is_some_and(|id| !id.is_string()) {
        return Err(unreadable(&format!("{at}.id")));
    }
    if let Some(size) = present(object.get("size")) {
        let size = size
            .as_object()
            .ok_or_else(|| unreadable(&format!("{at}.size")))?;
        for key in ["px", "weight"] {
            if present(size.get(key)).is_some_and(|n| !n.is_number()) {
                return Err(unreadable(&format!("{at}.size.{key}")));
            }
        }
    }
    match kind {
        "block" => match present(object.get("content")).map(Value::as_object) {
            Some(Some(content)) => match content.get("terminal").and_then(Value::as_str) {
                Some(address) => walk.terminals.push(address.to_owned()),
                // `mod`, `tab`, `agent` and contents newer than this build.
                _ => walk.other = true,
            },
            Some(None) => return Err(unreadable(&format!("{at}.content"))),
            None => walk.other = true,
        },
        "row" | "column" => {}
        // A stack, or a type newer than this build: kept, not drawn (H-4).
        _ => walk.other = true,
    }
    for (k, child) in array(object.get("children"), &format!("{at}.children"))?
        .iter()
        .enumerate()
    {
        walk_node(child, &format!("{at}.children[{k}]"), walk)?;
    }
    if let Some(floating) = present(object.get("floating")) {
        walk.other = true;
        // Floating entries that are nodes add their terminals after the tree;
        // their exact shape is the official app's, so others are skipped.
        for (k, float) in floating.as_array().into_iter().flatten().enumerate() {
            if float.get("type").is_some() {
                walk_node(float, &format!("{at}.floating[{k}]"), walk)?;
            }
        }
    }
    Ok(())
}

/// The container type holding both terminals of a two-terminal tree, when
/// every container holding both has that one type.
fn pair(root: &Value) -> Option<&str> {
    let kind = root.get("type").and_then(Value::as_str)?;
    if kind == "block" {
        return None;
    }
    let both = children(root).find(|child| count(child) == 2);
    match both {
        Some(child) => pair(child).filter(|inner| *inner == kind),
        None => Some(kind),
    }
}

fn count(node: &Value) -> usize {
    let own = usize::from(block_terminal(node).is_some());
    own + children(node).map(count).sum::<usize>()
}

fn children(node: &Value) -> impl Iterator<Item = &Value> {
    node.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn block_terminal(node: &Value) -> Option<&str> {
    (node.get("type")? == "block")
        .then(|| node.get("content")?.get("terminal")?.as_str())
        .flatten()
}

/// Absent and `null` both read as omitted.
fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|value| !value.is_null())
}

const EMPTY: &Vec<Value> = &Vec::new();

fn array<'a>(value: Option<&'a Value>, at: &str) -> Result<&'a Vec<Value>, DocError> {
    match present(value) {
        None => Ok(EMPTY),
        Some(value) => value.as_array().ok_or_else(|| unreadable(at)),
    }
}

fn text(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, DocError> {
    match present(object.get(key)) {
        None => Ok(String::new()),
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| unreadable(&format!("{at}.{key}"))),
    }
}

fn unreadable(at: &str) -> DocError {
    DocError::Unreadable(if at.is_empty() { "the top" } else { at }.to_owned())
}

/// The tab object a new terminal opens in (Design C, `NewTab`).
fn new_tab(tab_id: &str, pane_id: &str, terminal: &str) -> Value {
    json!({"id": tab_id, "name": "", "root": {"id": tab_id, "type": "row", "children": [
        {"id": pane_id, "type": "block", "content": {"terminal": terminal}}
    ]}})
}

/// `intent` applied to `value`; nothing else in it changes (Design D-1).
pub(crate) fn apply(value: &Value, intent: &Intent) -> Result<Applied, DocError> {
    let view = parse_view(value)?;
    let mut next = value.clone();
    let holds = |address: &str| {
        view.spaces
            .iter()
            .flat_map(|space| &space.tabs)
            .any(|tab| tab.terminals.iter().any(|known| known == address))
    };
    match intent {
        Intent::RenameSpace { id, name } => {
            let Some(i) = view.spaces.iter().position(|space| space.id == *id) else {
                return Ok(Applied::Unchanged);
            };
            Ok(rename(value, next, &["spaces".into(), i.into()], name))
        }
        Intent::RenameTab { id, name } => {
            let Some((i, j)) = tab_index(&view, |tab| tab.id == *id) else {
                return Ok(Applied::Unchanged);
            };
            let at = ["spaces".into(), i.into(), "tabs".into(), j.into()];
            Ok(rename(value, next, &at, name))
        }
        Intent::NewTab {
            space,
            tab_id,
            pane_id,
            terminal,
        } => {
            if holds(terminal) {
                return Ok(Applied::Unchanged);
            }
            let i = view
                .spaces
                .iter()
                .position(|known| known.id == *space)
                .ok_or(DocError::Missing("space"))?;
            let tab = new_tab(tab_id, pane_id, terminal);
            let space = locate_mut(&mut next, &["spaces".into(), i.into()])
                .ok_or(DocError::Missing("space"))?;
            push(space, "tabs", tab)?;
            Ok(Applied::Changed(next))
        }
        Intent::NewSpace {
            space_id,
            folder,
            tab_id,
            pane_id,
            terminal,
        } => {
            if holds(terminal) {
                return Ok(Applied::Unchanged);
            }
            let tabs = vec![new_tab(tab_id, pane_id, terminal)];
            let space = json!({"id": space_id, "name": "", "folder": folder, "tabs": tabs});
            if next.is_null() {
                next = json!({});
            }
            push(&mut next, "spaces", space)?;
            Ok(Applied::Changed(next))
        }
        Intent::PruneSpace { id, space, ended } => {
            let Some(i) = view.spaces.iter().position(|known| known.id == *id) else {
                return Ok(Applied::Unchanged);
            };
            let Some(Value::Array(spaces)) = next.get_mut("spaces") else {
                return Err(DocError::Missing("space"));
            };
            if spaces.get(i) != Some(space) {
                return Ok(Applied::Unchanged);
            }
            let pruned = pruned(space, ended);
            if pruned.as_ref() == Some(space) {
                return Ok(Applied::Unchanged);
            }
            match pruned {
                Some(pruned) => spaces[i] = pruned,
                None => {
                    spaces.remove(i);
                }
            }
            Ok(Applied::Changed(next))
        }
        Intent::RemoveSpace { id } => {
            let Some(i) = view.spaces.iter().position(|known| known.id == *id) else {
                return Ok(Applied::Unchanged);
            };
            let Some(Value::Array(spaces)) = next.get_mut("spaces") else {
                return Err(DocError::Missing("space"));
            };
            spaces.remove(i);
            Ok(Applied::Changed(next))
        }
        Intent::Migrate { space_id, tabs } => {
            // A terminal the document already names stays where it is, so
            // a migration that landed once adds nothing the second time.
            let tabs = tabs
                .iter()
                .filter_map(|tab| {
                    let panes = tab
                        .panes
                        .iter()
                        .filter(|(_, terminal)| !holds(terminal))
                        .map(|(pane, terminal)| {
                            json!({"id": pane, "type": "block", "content": {"terminal": terminal}})
                        })
                        .collect::<Vec<_>>();
                    let kind = if tab.vertical && panes.len() == 2 {
                        "column"
                    } else {
                        "row"
                    };
                    (!panes.is_empty()).then(|| {
                        json!({"id": tab.tab_id, "name": "", "root":
                            {"id": tab.tab_id, "type": kind, "children": panes}})
                    })
                })
                .collect::<Vec<_>>();
            if tabs.is_empty() {
                return Ok(Applied::Unchanged);
            }
            let space = json!({"id": space_id, "name": MIGRATED_SPACE, "folder": "", "tabs": tabs});
            if next.is_null() {
                next = json!({});
            }
            push(&mut next, "spaces", space)?;
            Ok(Applied::Changed(next))
        }
        Intent::Split {
            terminal,
            axis,
            new_pane,
            new_container,
            new_terminal,
        } => {
            if holds(new_terminal) {
                return Ok(Applied::Unchanged);
            }
            let (i, j) = tab_index(&view, |tab| tab.terminals.iter().any(|t| t == terminal))
                .ok_or(DocError::Missing("terminal"))?;
            if view.spaces[i].tabs[j].drawable != Drawable::One {
                return Err(DocError::Undrawable);
            }
            let at = [
                "spaces".into(),
                i.into(),
                "tabs".into(),
                j.into(),
                "root".into(),
            ];
            let root = locate_mut(&mut next, &at).ok_or(DocError::Missing("terminal"))?;
            let block = json!({"id": new_pane, "type": "block",
                "content": {"terminal": new_terminal}});
            split(root, terminal, *axis, block, new_container)?;
            Ok(Applied::Changed(next))
        }
        #[cfg(test)]
        Intent::Replace(candidate) => Ok(if candidate == value {
            Applied::Unchanged
        } else {
            Applied::Changed(candidate.clone())
        }),
        #[cfg(test)]
        Intent::RenameSpaceAlsoTouch { id, name } => {
            let Some(i) = view.spaces.iter().position(|space| space.id == *id) else {
                return Ok(Applied::Unchanged);
            };
            for at in [
                vec!["spaces".into(), i.into()],
                vec!["spaces".into(), i.into(), "tabs".into(), 0.into()],
            ] {
                if let Some(Value::Object(item)) = locate_mut(&mut next, &at) {
                    item.insert("name".into(), json!(name));
                }
            }
            Ok(Applied::Changed(next))
        }
    }
}

fn tab_index(view: &View, find: impl Fn(&TabView) -> bool) -> Option<(usize, usize)> {
    view.spaces.iter().enumerate().find_map(|(i, space)| {
        let j = space.tabs.iter().position(&find)?;
        Some((i, j))
    })
}

/// Sets the `name` under `at` to `name` trimmed; `""` is the automatic name.
fn rename(value: &Value, mut next: Value, at: &[Step], name: &str) -> Applied {
    let name = name.trim();
    let current = locate(value, at).and_then(|item| present(item.get("name")));
    if current.and_then(Value::as_str).unwrap_or_default() == name
        && (current.is_some() || name.is_empty())
    {
        return Applied::Unchanged;
    }
    if let Some(Value::Object(item)) = locate_mut(&mut next, at) {
        item.insert("name".into(), json!(name));
    }
    Applied::Changed(next)
}

/// Appends `item` to the array under `key`, made when absent.
fn push(object: &mut Value, key: &str, item: Value) -> Result<(), DocError> {
    let object = object.as_object_mut().ok_or_else(|| unreadable(key))?;
    match object.get_mut(key) {
        Some(Value::Array(items)) => items.push(item),
        _ => {
            object.insert(key.into(), Value::Array(vec![item]));
        }
    }
    Ok(())
}

/// The official app's split (Design C; issue D4 rule): beside the block in a
/// parent of the same direction, halving a `weight`; otherwise the block is
/// wrapped, with its size, in a new container of the split's direction.
fn split(
    root: &mut Value,
    terminal: &str,
    axis: Axis,
    mut block: Value,
    container: &str,
) -> Result<(), DocError> {
    if block_terminal(root) == Some(terminal) {
        // A root block has the tab's id; wrapping it would move that id.
        return Err(DocError::Undrawable);
    }
    let kind = root.get("type").and_then(Value::as_str).map(str::to_owned);
    let Some(Value::Array(children)) = root.get_mut("children") else {
        return Err(DocError::Missing("terminal"));
    };
    let Some(k) = children
        .iter()
        .position(|child| block_terminal(child) == Some(terminal))
    else {
        for child in children.iter_mut() {
            if count(child) > 0 {
                return split(child, terminal, axis, block, container);
            }
        }
        return Err(DocError::Missing("terminal"));
    };
    if kind.as_deref() == Some(axis.as_str()) {
        let half = children[k]
            .get("size")
            .and_then(|size| size.get("weight"))
            .and_then(Value::as_f64)
            .and_then(|weight| serde_json::Number::from_f64(weight / 2.0));
        if let Some(half) = half {
            if let Some(size) = children[k].get_mut("size").and_then(Value::as_object_mut) {
                size.insert("weight".into(), json!(half));
            }
            block["size"] = json!({"weight": half});
        }
        children.insert(k + 1, block);
    } else {
        let mut old = children[k].take();
        let mut wrap = json!({"id": container, "type": axis.as_str()});
        if let Some(size) = old.as_object_mut().and_then(|old| old.shift_remove("size")) {
            wrap["size"] = size;
        }
        wrap["children"] = json!([old, block]);
        children[k] = wrap;
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum Step {
    Key(&'static str),
    Index(usize),
}

impl From<&'static str> for Step {
    fn from(key: &'static str) -> Self {
        Self::Key(key)
    }
}

impl From<usize> for Step {
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

fn locate<'a>(value: &'a Value, at: &[Step]) -> Option<&'a Value> {
    at.iter().try_fold(value, |value, step| match step {
        Step::Key(key) => value.get(*key),
        Step::Index(index) => value.get(*index),
    })
}

fn locate_mut<'a>(value: &'a mut Value, at: &[Step]) -> Option<&'a mut Value> {
    at.iter().try_fold(value, |value, step| match step {
        Step::Key(key) => value.get_mut(*key),
        Step::Index(index) => value.get_mut(*index),
    })
}

/// The gates of Design D-4, in order; `Err` means nothing is sent.
pub(crate) fn check_write(
    original: &Value,
    candidate: &Value,
    intent: &Intent,
) -> Result<(), WriteCheck> {
    let bytes = serde_json::to_vec(candidate).map_err(|_| WriteCheck::Unreadable)?;
    if bytes.len() > MAX_BYTES {
        return Err(WriteCheck::TooLarge);
    }
    parse_view(candidate).map_err(|_| WriteCheck::Unreadable)?;
    if !only_intended(original, candidate, intent) {
        return Err(WriteCheck::OutsidePath);
    }
    ids(candidate)
}

/// (c): undoing the intent's own change leaves the original, byte for byte.
fn only_intended(original: &Value, candidate: &Value, intent: &Intent) -> bool {
    let Ok(view) = parse_view(original) else {
        return false;
    };
    let mut undone = candidate.clone();
    let restored = match intent {
        Intent::RenameSpace { id, .. } => view
            .spaces
            .iter()
            .position(|space| space.id == *id)
            .is_some_and(|i| restore(original, &mut undone, &["spaces".into(), i.into()])),
        Intent::RenameTab { id, .. } => {
            tab_index(&view, |tab| tab.id == *id).is_some_and(|(i, j)| {
                let at = ["spaces".into(), i.into(), "tabs".into(), j.into()];
                restore(original, &mut undone, &at)
            })
        }
        Intent::NewTab { space, .. } => view
            .spaces
            .iter()
            .position(|known| known.id == *space)
            .is_some_and(|i| unpush(original, &mut undone, &["spaces".into(), i.into()], "tabs")),
        Intent::PruneSpace { id, ended, .. } => view
            .spaces
            .iter()
            .position(|space| space.id == *id)
            .is_some_and(|i| unprune(original, &mut undone, i, ended)),
        Intent::RemoveSpace { id } => view
            .spaces
            .iter()
            .position(|space| space.id == *id)
            .is_some_and(|i| unremove(original, &mut undone, i)),
        Intent::NewSpace { .. } | Intent::Migrate { .. } => {
            let undone_top = unpush(original, &mut undone, &[], "spaces");
            if original.is_null() && undone.as_object().is_some_and(Map::is_empty) {
                undone = Value::Null;
            }
            undone_top
        }
        Intent::Split { terminal, .. } => {
            tab_index(&view, |tab| tab.terminals.iter().any(|t| t == terminal)).is_some_and(
                |(i, j)| {
                    let at = [
                        "spaces".into(),
                        i.into(),
                        "tabs".into(),
                        j.into(),
                        "root".into(),
                    ];
                    match (locate(original, &at), locate_mut(&mut undone, &at)) {
                        (Some(old), Some(new)) => {
                            *new = old.clone();
                            true
                        }
                        _ => false,
                    }
                },
            )
        }
        #[cfg(test)]
        Intent::Replace(_) => return true,
        #[cfg(test)]
        Intent::RenameSpaceAlsoTouch { id, .. } => view
            .spaces
            .iter()
            .position(|space| space.id == *id)
            .is_some_and(|i| restore(original, &mut undone, &["spaces".into(), i.into()])),
    };
    restored && serde_json::to_vec(&undone).ok() == serde_json::to_vec(original).ok()
}

/// Puts the original `name` back under `at` (or removes an added one).
fn restore(original: &Value, undone: &mut Value, at: &[Step]) -> bool {
    let old = locate(original, at)
        .and_then(|item| item.get("name"))
        .cloned();
    let Some(Value::Object(item)) = locate_mut(undone, at) else {
        return false;
    };
    match old {
        Some(old) => {
            item.insert("name".into(), old);
        }
        None => {
            item.shift_remove("name");
        }
    }
    true
}

/// The space without its ended panes, or `None` when no tab is left.
fn pruned(space: &Value, ended: &[String]) -> Option<Value> {
    let ended = ended.iter().cloned().collect();
    let next = super::herdr_doc_prune::prune_space(space, &ended);
    let left = next
        .get("tabs")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    (left > 0).then_some(next)
}

/// Undoes a prune of the original's space `i`: the candidate must hold
/// exactly that space pruned (or no space there when none was left), and
/// it is put back as it was.
fn unprune(original: &Value, undone: &mut Value, i: usize, ended: &[String]) -> bool {
    let Some(space) = locate(original, &["spaces".into(), i.into()]).cloned() else {
        return false;
    };
    let expected = pruned(&space, ended);
    let Some(Value::Array(spaces)) = undone.get_mut("spaces") else {
        return false;
    };
    match expected {
        Some(expected) => match spaces.get_mut(i) {
            Some(slot) if *slot == expected => {
                *slot = space;
                true
            }
            _ => false,
        },
        None if i <= spaces.len() => {
            spaces.insert(i, space);
            true
        }
        None => false,
    }
}

/// Undoes a removal of the original's space `i`: it is put back where it
/// was, so the candidate must hold every other space as it was.
fn unremove(original: &Value, undone: &mut Value, i: usize) -> bool {
    let Some(space) = locate(original, &["spaces".into(), i.into()]).cloned() else {
        return false;
    };
    match undone.get_mut("spaces") {
        Some(Value::Array(spaces)) if i <= spaces.len() => {
            spaces.insert(i, space);
            true
        }
        _ => false,
    }
}

/// Removes the one item appended to the array `key` under `at`.
fn unpush(original: &Value, undone: &mut Value, at: &[Step], key: &str) -> bool {
    let old = locate(original, at).and_then(|item| item.get(key)).cloned();
    let Some(Value::Object(item)) = locate_mut(undone, at) else {
        return false;
    };
    let Some(Value::Array(items)) = item.get_mut(key) else {
        return false;
    };
    if items.pop().is_none() {
        return false;
    }
    if items.is_empty() {
        // An absent or `null` array was made by the push.
        match old {
            None => {
                item.shift_remove(key);
            }
            Some(Value::Null) => {
                item.insert(key.into(), Value::Null);
            }
            Some(_) => {}
        }
    }
    true
}

/// (d): each tab's root has the tab's id, and node ids are unique in a tab.
fn ids(candidate: &Value) -> Result<(), WriteCheck> {
    let tabs = candidate
        .get("spaces")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|space| space.get("tabs")?.as_array())
        .flatten();
    for tab in tabs {
        let Some(root) = present(tab.get("root")) else {
            continue;
        };
        if root.get("id") != tab.get("id") {
            return Err(WriteCheck::RootId);
        }
        let mut seen = BTreeSet::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if let Some(id) = node.get("id").and_then(Value::as_str)
                && !seen.insert(id)
            {
                return Err(WriteCheck::DuplicatePane);
            }
            stack.extend(children(node));
            stack.extend(
                node.get("floating")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten(),
            );
        }
    }
    Ok(())
}

/// A new id the official app's way: the prefix (`sp` space, `tb` tab, `p`
/// pane, `s` container) and 128 random bits in hexadecimal.
pub(crate) fn new_id(prefix: &str) -> Result<String, DocError> {
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut bytes))
        .map_err(DocError::Random)?;
    Ok(format!("{prefix}{:032x}", u128::from_be_bytes(bytes)))
}
