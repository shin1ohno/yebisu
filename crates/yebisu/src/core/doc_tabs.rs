//! The tabs a `default` session draws while it reads the `herdr-app`
//! document (issue #144, Design F): those of the selected document space,
//! or the iPad-only tabs, whose terminals are in no document block.
//!
//! The worker keeps one `Workspace` to draw from. While a document space is
//! drawn, its tabs are built from the document and the iPad-only tabs wait
//! here. The iPad file keeps the iPad-only tabs and the iPad's own choice of
//! space and tab (`doc_view`, decision 11), never document tabs.
use super::{
    DocCatalog, DocRows, DocSpace, SessionInfo, TabInfo,
    herdr_doc::{Axis, Drawable, Intent, MigratedTab, SpaceView, View, new_id},
    herdr_doc_sync::{DocSnapshot, DocWriter, Read, WriteStats},
    store::{Saved, Tab, Workspace},
};
use crate::shell::herdr2_frame::{names, session::one_line};
use serde_json::{Value, json};
use sheprd_backend::{Spawn, TerminalId};
use sheprd_core_client::{CoreBackend, wire::Run};
use std::collections::{BTreeMap, HashSet};

/// Why New space is refused: the document cannot be written (Q6).
const NO_DOCUMENT: &str = "Can't read the herdr-app layout; New space needs it";
/// Why a split is refused: the tree is not one yebisu draws (Design F).
const NOT_SPLITTABLE: &str = "This tab's herdr-app layout can't be split on the iPad";

enum Taken {
    Readable {
        view: View,
        live: HashSet<String>,
        names: BTreeMap<String, String>,
    },
    Unreadable,
}

/// What the worker last took from the document.
#[derive(Default)]
pub(super) struct DocTabs {
    /// `None` before the first answer (and after failed reads only): the
    /// session then draws as if there were no document.
    taken: Option<Taken>,
    version: u64,
    /// The iPad-only tabs and their selection while a space is drawn.
    local: Option<(Vec<Tab>, usize)>,
    /// A terminal past a tab's first two that was focused: that tab draws
    /// it alone (Design F, "the rest by switching").
    shown: Option<(String, String)>,
    /// A space this iPad created, drawn as soon as a read has it.
    pending: Option<String>,
    stats: WriteStats,
    /// The intents whose outcome was already told.
    told: u64,
    /// The migration in flight: its new space, and the intents finished
    /// when it was queued (Design G).
    migrating: Option<(String, u64)>,
    /// A migration failed on this connection; the next one tries again.
    tried: bool,
}

impl DocTabs {
    /// Takes a read. True when it changed what the session shows; the
    /// caller attaches again when `drawn` changed.
    pub(super) fn take(&mut self, snapshot: DocSnapshot, workspace: &mut Workspace) -> bool {
        self.taken = match snapshot.read {
            Read::Readable(view) => Some(Taken::Readable {
                view,
                live: snapshot.live.into_iter().collect(),
                names: snapshot.names,
            }),
            Read::Unreadable => Some(Taken::Unreadable),
            // A failed read keeps what is drawn (writes are D4's to refuse).
            Read::Unavailable(_) => return false,
        };
        self.version = snapshot.version;
        self.stats = snapshot.stats;
        self.prune(workspace);
        self.settle_migration(workspace);
        let pending = self.pending.take();
        let space = match pending.clone().filter(|id| self.space(id).is_some()) {
            Some(created) => Some(created),
            None => {
                self.pending = pending_again(pending, &self.taken);
                workspace.doc_view.space.clone()
            }
        };
        let space = match space.filter(|id| self.space(id).is_some()) {
            Some(space) => Some(space),
            None => self.first(workspace),
        };
        self.draw(space, workspace);
        true
    }

    /// Whether the iPad's saved `default` tabs should move into the document
    /// now: it is readable, the tabs are there, and no migration ran before
    /// or is running (Design G, AC12).
    pub(super) fn migration_due(&self, workspace: &Workspace) -> bool {
        workspace.migrated.is_none()
            && self.migrating.is_none()
            && !self.tried
            && self.view().is_some()
            && self.local_tabs(workspace) > 0
    }

    /// Queues the one intent that moves every iPad-only tab into a new
    /// space "iPad" (decision 4), after `backup` kept the iPad file. The
    /// terminals keep no labels (Design G). Tried once per connection.
    pub(super) fn migrate(
        &mut self,
        writer: Option<&DocWriter>,
        workspace: &Workspace,
        backup: impl FnOnce() -> crate::Result<()>,
    ) -> crate::Result<()> {
        self.tried = true;
        let writer = writer.ok_or(crate::Error::CoreQueue)?;
        backup()?;
        let tabs = match &self.local {
            Some((tabs, _)) => tabs,
            None => &workspace.tabs,
        };
        let tabs = tabs
            .iter()
            .map(|tab| {
                let panes = tab.layout.ids.iter().map(|terminal| {
                    Ok::<_, crate::Error>((id("p")?, terminal.as_str().to_owned()))
                });
                Ok(MigratedTab {
                    tab_id: id("tb")?,
                    panes: panes.collect::<crate::Result<_>>()?,
                    vertical: tab.layout.vertical,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let space = id("sp")?;
        writer.send(Intent::Migrate {
            space_id: space.clone(),
            tabs,
        })?;
        self.migrating = Some((space, self.stats.finished));
        Ok(())
    }

    /// Records the migration once the document shows it: its space, or no
    /// iPad-only tab left (a migration that landed before). A migration
    /// that ended otherwise stays undone until the next connection.
    fn settle_migration(&mut self, workspace: &mut Workspace) {
        if workspace.migrated.is_some() || self.view().is_none() {
            return;
        }
        let none_left = self.local_tabs(workspace) == 0;
        match self.migrating.clone() {
            Some((space, _)) if self.space(&space).is_some() || none_left => {
                workspace.migrated = Some(self.version);
                self.pending = Some(space);
                self.migrating = None;
            }
            Some((_, before)) if self.stats.finished > before => {
                self.migrating = None;
                self.tried = true;
            }
            Some(_) => {}
            // Nothing saved to move: there is nothing to migrate later.
            None if none_left && !self.tried => workspace.migrated = Some(self.version),
            None => {}
        }
    }

    /// The notice of an intent that was not saved, once per intent.
    pub(super) fn notice(&mut self) -> Option<String> {
        if self.stats.finished == self.told {
            return None;
        }
        self.told = self.stats.finished;
        self.stats.notice.clone()
    }

    /// New tab in the drawn document space (Design C, `NewTab`): a
    /// terminal with the official app's labels, then the intent. The tab is
    /// drawn when the document read after the write has it.
    pub(super) fn new_tab(
        &mut self,
        backend: &CoreBackend,
        writer: Option<&DocWriter>,
        workspace: &mut Workspace,
    ) -> crate::Result<()> {
        let space = workspace
            .doc_view
            .space
            .clone()
            .filter(|_| self.local.is_some())
            .ok_or(crate::Error::CoreState)?;
        let writer = writer.ok_or(crate::Error::CoreQueue)?;
        let (tab, pane) = (id("tb")?, id("p")?);
        let terminal = run(backend, &space, &tab)?;
        writer.send(Intent::NewTab {
            space: space.clone(),
            tab_id: tab.clone(),
            pane_id: pane,
            terminal: terminal.address,
        })?;
        workspace.doc_view.tabs.insert(space, tab);
        Ok(())
    }

    /// Herdr 2's New space (decision 10): a document space whose folder is
    /// the new terminal's. Refused while the document cannot be written.
    pub(super) fn new_space(
        &mut self,
        backend: &CoreBackend,
        writer: Option<&DocWriter>,
        workspace: &mut Workspace,
    ) -> crate::Result<()> {
        let writer = writer
            .filter(|_| self.view().is_some())
            .ok_or_else(|| crate::Error::Ssh(NO_DOCUMENT.into()))?;
        let (space, tab, pane) = (id("sp")?, id("tb")?, id("p")?);
        let terminal = run(backend, &space, &tab)?;
        writer.send(Intent::NewSpace {
            space_id: space.clone(),
            folder: terminal.cwd,
            tab_id: tab.clone(),
            pane_id: pane,
            terminal: terminal.address,
        })?;
        workspace.doc_view.tabs.insert(space.clone(), tab);
        self.pending = Some(space);
        Ok(())
    }

    /// The terminal a split starts: with the drawn tab's labels in a
    /// document space, which splits only a one-pane tree (Design F).
    pub(super) fn split_spawn(&self, workspace: &Workspace) -> crate::Result<Spawn> {
        if self.local.is_none() {
            return Ok(Spawn::default());
        }
        let (space, tab) = self.drawn_ids(workspace).ok_or(crate::Error::CoreState)?;
        let one = self
            .space(&space)
            .and_then(|view| view.tabs.iter().find(|doc| doc.id == tab))
            .is_some_and(|doc| doc.drawable == Drawable::One && self.shown.is_none());
        if !one {
            return Err(crate::Error::Ssh(NOT_SPLITTABLE.into()));
        }
        let labels = labels(&space, &tab);
        Ok(Spawn {
            labels: labels
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
            ..Spawn::default()
        })
    }

    /// The split's intent, once its terminal runs (Design C, `Split`).
    pub(super) fn split_written(
        &self,
        writer: Option<&DocWriter>,
        first: &TerminalId,
        new: &TerminalId,
        vertical: bool,
    ) -> crate::Result<()> {
        if self.local.is_none() {
            return Ok(());
        }
        writer.ok_or(crate::Error::CoreQueue)?.send(Intent::Split {
            terminal: first.as_str().to_owned(),
            axis: if vertical { Axis::Column } else { Axis::Row },
            new_pane: id("p")?,
            new_container: id("s")?,
            new_terminal: new.as_str().to_owned(),
        })
    }

    fn drawn_ids(&self, workspace: &Workspace) -> Option<(String, String)> {
        let space = workspace.doc_view.space.clone()?;
        let tab = workspace.tabs.get(workspace.active)?.id.clone();
        Some((space, tab))
    }

    /// Draws document space `space`, or the iPad-only tabs for `None`.
    pub(super) fn select(&mut self, space: Option<String>, workspace: &mut Workspace) {
        self.shown = None;
        let space = space.filter(|id| self.space(id).is_some());
        self.draw(space, workspace);
    }

    /// Whether a document space is drawn (`NewTab` and `Split` wait).
    pub(super) fn drawing_space(&self) -> bool {
        self.local.is_some()
    }

    /// Draws what holds `address` for a focus: its document space, or the
    /// iPad-only tabs, which a terminal in no tab also joins. True when the
    /// drawn tabs changed.
    pub(super) fn focus(&mut self, address: &str, workspace: &mut Workspace) -> bool {
        let before = drawn(workspace);
        self.reveal(address, workspace);
        let held = workspace
            .tabs
            .iter()
            .any(|tab| tab.layout.ids.iter().any(|id| id.as_str() == address));
        if !held && self.drawing_space() {
            self.select(None, workspace);
        }
        before != drawn(workspace)
    }

    /// The session's catalog row gets the document's spaces, and the drawn
    /// document tabs say which have no name of their own.
    /// It also writes `qa.state.herdr_doc`, so the QA field changes with
    /// the rows (a session that never read the document writes nothing).
    pub(super) fn fill(&self, info: &mut SessionInfo, workspace: &Workspace) {
        info.doc = self.catalog(workspace);
        if self.taken.is_some() {
            crate::qa_ax::core::doc(|| self.qa_value(workspace));
        }
        let Some(DocCatalog {
            state: DocRows::Readable(spaces),
            selected: Some(selected),
            ..
        }) = &info.doc
        else {
            return;
        };
        if let Some(space) = spaces.iter().find(|space| space.id == *selected) {
            for tab in &mut info.tabs {
                tab.auto_name = space.tabs.iter().any(|t| t.id == tab.id && t.auto_name);
            }
        }
    }

    /// Draws the space or the iPad-only tabs that hold `address`, if they
    /// are not drawn; true when the drawn tabs changed.
    pub(super) fn reveal(&mut self, address: &str, workspace: &mut Workspace) -> bool {
        let found = self.view().and_then(|view| {
            view.spaces.iter().find_map(|space| {
                let tab = space
                    .tabs
                    .iter()
                    .find(|tab| tab.terminals.iter().any(|t| t == address))?;
                Some((space.id.clone(), tab.id.clone()))
            })
        });
        let before = drawn(workspace);
        match found {
            Some((space, tab)) => {
                workspace.doc_view.tabs.insert(space.clone(), tab.clone());
                let hidden = workspace
                    .tabs
                    .iter()
                    .find(|t| t.id == tab)
                    .is_none_or(|t| !t.layout.ids.iter().any(|id| id.as_str() == address));
                self.shown = hidden.then(|| (tab, address.to_owned()));
                self.draw(Some(space), workspace);
            }
            None if self.local.is_some() && self.is_local(address) => {
                self.select(None, workspace);
            }
            None => return false,
        }
        before != drawn(workspace)
    }

    fn is_local(&self, address: &str) -> bool {
        self.local.as_ref().is_some_and(|(tabs, _)| {
            tabs.iter()
                .any(|tab| tab.layout.ids.iter().any(|id| id.as_str() == address))
        })
    }

    /// Every terminal in a document block (R4: never an iPad-only tab).
    fn blocks(&self) -> HashSet<String> {
        self.view()
            .into_iter()
            .flat_map(|view| &view.spaces)
            .flat_map(|space| &space.tabs)
            .flat_map(|tab| tab.terminals.iter().cloned())
            .collect()
    }

    /// What the iPad file keeps: the iPad-only tabs, not document tabs.
    pub(super) fn saved(&self, workspace: &Workspace) -> Workspace {
        let (tabs, active) = match &self.local {
            Some((tabs, active)) => (tabs.clone(), *active),
            None => (workspace.tabs.clone(), workspace.active),
        };
        Workspace {
            created: workspace.created,
            tabs,
            active,
            doc_view: workspace.doc_view.clone(),
            migrated: workspace.migrated,
        }
    }

    /// Remembers the drawn tab of the drawn space.
    pub(super) fn note_tab(&mut self, workspace: &mut Workspace) {
        if self.shown.as_ref().map(|(tab, _)| tab)
            != workspace.tabs.get(workspace.active).map(|tab| &tab.id)
        {
            self.shown = None;
        }
        if let (true, Some(space), Some(tab)) = (
            self.local.is_some(),
            workspace.doc_view.space.clone(),
            workspace
                .tabs
                .get(workspace.active)
                .map(|tab| tab.id.clone()),
        ) {
            workspace.doc_view.tabs.insert(space, tab);
        }
    }

    /// The document part of the session's catalog row.
    pub(super) fn catalog(&self, workspace: &Workspace) -> Option<DocCatalog> {
        let state = match self.taken.as_ref()? {
            Taken::Unreadable => DocRows::Unreadable,
            Taken::Readable { view, names, .. } => DocRows::Readable(
                view.spaces
                    .iter()
                    .filter(|space| !space.id.is_empty())
                    .map(|space| doc_space(space, names))
                    .collect(),
            ),
        };
        Some(DocCatalog {
            state,
            selected: workspace
                .doc_view
                .space
                .clone()
                .filter(|_| self.local.is_some()),
            local_tabs: self.local_tabs(workspace),
        })
    }

    /// `qa.state.herdr_doc` (#144 Simulator QA). D4 adds the writes.
    pub(super) fn qa_value(&self, workspace: &Workspace) -> Value {
        let catalog = self.catalog(workspace);
        let (state, spaces) = match catalog.as_ref().map(|catalog| &catalog.state) {
            None => (Value::Null, Vec::new()),
            Some(DocRows::Unreadable) => (json!("unreadable"), Vec::new()),
            Some(DocRows::Readable(spaces)) => (
                json!("readable"),
                spaces
                    .iter()
                    .map(|s| json!({"id": s.id, "name": s.name, "tabs": s.tabs.len()}))
                    .collect(),
            ),
        };
        json!({"state": state, "version": self.version, "spaces": spaces,
            "selected": catalog.and_then(|catalog| catalog.selected),
            "local_tabs": self.local_tabs(workspace),
            "writes": self.stats.writes, "conflicts": self.stats.conflicts,
            "last": self.stats.last})
    }

    fn local_tabs(&self, workspace: &Workspace) -> usize {
        self.local
            .as_ref()
            .map_or(workspace.tabs.len(), |(tabs, _)| tabs.len())
    }

    fn view(&self) -> Option<&View> {
        match self.taken.as_ref()? {
            Taken::Readable { view, .. } => Some(view),
            Taken::Unreadable => None,
        }
    }

    fn space(&self, id: &str) -> Option<&SpaceView> {
        self.view()?.spaces.iter().find(|space| space.id == id)
    }

    /// Owner answer D2-1 (4): the iPad-only tabs when there are any,
    /// otherwise the first document space.
    fn first(&self, workspace: &Workspace) -> Option<String> {
        if self.local_tabs(workspace) > 0 {
            return None;
        }
        let space = self
            .view()?
            .spaces
            .iter()
            .find(|space| !space.id.is_empty());
        space.map(|space| space.id.clone())
    }

    /// Drops document terminals from the iPad-only tabs.
    fn prune(&mut self, workspace: &mut Workspace) {
        let blocks = self.blocks();
        let (tabs, active) = match &mut self.local {
            Some((tabs, active)) => (tabs, active),
            None => (&mut workspace.tabs, &mut workspace.active),
        };
        for tab in tabs.iter_mut() {
            let focus = tab.layout.ids.get(tab.layout.active).cloned();
            tab.layout.ids.retain(|id| !blocks.contains(id.as_str()));
            tab.layout.active = tab
                .layout
                .ids
                .iter()
                .position(|id| Some(id) == focus.as_ref())
                .unwrap_or(0);
        }
        tabs.retain(|tab| !tab.layout.ids.is_empty());
        *active = (*active).min(tabs.len().saturating_sub(1));
    }

    fn draw(&mut self, space: Option<String>, workspace: &mut Workspace) {
        let built = space.as_deref().and_then(|id| self.space(id)).map(|view| {
            let tabs = self.space_tabs(view, workspace);
            let chosen = workspace.doc_view.tabs.get(&view.id);
            let active = chosen
                .and_then(|id| tabs.iter().position(|tab| tab.id == *id))
                .unwrap_or(0);
            (tabs, active)
        });
        match built {
            Some((tabs, active)) => {
                if self.local.is_none() {
                    self.local = Some((std::mem::take(&mut workspace.tabs), workspace.active));
                }
                workspace.tabs = tabs;
                workspace.active = active;
                workspace.doc_view.space = space;
            }
            None => {
                if let Some((tabs, active)) = self.local.take() {
                    workspace.tabs = tabs;
                    workspace.active = active;
                }
                workspace.doc_view.space = None;
            }
        }
    }

    /// A tab per document tab with a live terminal: its first two live
    /// terminals in depth-first order (Design F). The pane focus is kept,
    /// and so is the direction of a tree yebisu cannot draw.
    fn space_tabs(&self, view: &SpaceView, workspace: &Workspace) -> Vec<Tab> {
        let Some(Taken::Readable { live, .. }) = &self.taken else {
            return Vec::new();
        };
        view.tabs
            .iter()
            .enumerate()
            .filter_map(|(index, doc)| {
                let alive = doc
                    .terminals
                    .iter()
                    .filter(|address| live.contains(*address))
                    .filter_map(|address| TerminalId::new(address.clone()).ok());
                let ids = match &self.shown {
                    Some((tab, address)) if *tab == doc.id => alive
                        .filter(|id| id.as_str() == address)
                        .take(1)
                        .collect::<Vec<_>>(),
                    _ => alive.take(2).collect(),
                };
                if ids.is_empty() || doc.id.is_empty() {
                    return None;
                }
                let kept = workspace
                    .tabs
                    .iter()
                    .find(|tab| tab.id == doc.id && tab.layout.ids == ids);
                let vertical = match doc.drawable {
                    Drawable::Two { vertical } => vertical,
                    _ => kept.is_some_and(|tab| tab.layout.vertical),
                };
                Some(Tab {
                    id: doc.id.clone(),
                    label: tab_name(&doc.name, index),
                    layout: Saved {
                        active: kept.map_or(0, |tab| tab.layout.active.min(ids.len() - 1)),
                        ids,
                        vertical,
                    },
                })
            })
            .collect()
    }
}

/// A created space not in this read keeps waiting while the document is
/// readable; an unreadable one cannot have it.
fn pending_again(pending: Option<String>, taken: &Option<Taken>) -> Option<String> {
    pending.filter(|_| matches!(taken, Some(Taken::Readable { .. })))
}

fn id(prefix: &str) -> crate::Result<String> {
    new_id(prefix).map_err(|error| crate::Error::Ssh(error.to_string()))
}

/// The labels the official app gives its own terminals (Design C), so it
/// takes this one into the same space and tab even before the write lands.
fn labels<'a>(space: &'a str, tab: &'a str) -> BTreeMap<&'static str, &'a str> {
    BTreeMap::from([
        ("herdr.space", space),
        ("herdr.tab", tab),
        ("herdr.seen", "0"),
    ])
}

fn run(
    backend: &CoreBackend,
    space: &str,
    tab: &str,
) -> crate::Result<sheprd_core_client::wire::Terminal> {
    Ok(backend.core().run(&Run {
        keep: true,
        labels: labels(space, tab),
        ..Run::default()
    })?)
}

/// The drawn tab's id and panes, to tell whether it must attach again.
pub(super) fn drawn(workspace: &Workspace) -> Option<(String, Vec<String>)> {
    let tab = workspace.tabs.get(workspace.active)?;
    let ids = tab.layout.ids.iter().map(|id| id.as_str().to_owned());
    Some((tab.id.clone(), ids.collect()))
}

/// Design E: a named tab shows its name, an unnamed one its position.
fn tab_name(name: &str, index: usize) -> String {
    let name = one_line(name);
    if name.is_empty() {
        (index + 1).to_string()
    } else {
        name
    }
}

fn doc_space(space: &SpaceView, names: &BTreeMap<String, String>) -> DocSpace {
    let name = one_line(&space.name);
    let auto_name = name.is_empty();
    DocSpace {
        id: space.id.clone(),
        name: if auto_name {
            names::display(&space.folder, names)
        } else {
            name
        },
        auto_name,
        folder: space.folder.clone(),
        tabs: space
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| TabInfo {
                id: tab.id.clone(),
                label: tab_name(&tab.name, index),
                auto_name: one_line(&tab.name).is_empty(),
                panes: tab.terminals.clone(),
                active: false,
                focused_pane: 0,
                vertical: matches!(tab.drawable, Drawable::Two { vertical: true }),
            })
            .collect(),
    }
}
