//! Rename on Herdr 2 (issue #144 D3): the spaces and tabs of the `default`
//! session's `herdr-app` document, written through the document (Design E,
//! `Command::RenameSpace` / `RenameTab`). Herdr 2's snapshot is synthesized
//! and holds no spaces or tabs, so the field's first text, the check that
//! the object still exists and the optimistic label read what the drawn
//! `default` session last read of the document instead. Panes stay
//! unrenamable on Herdr 2 (decision 8, AC14).
//!
//! A space keeps the row's identity (`default\0<space id>`), as the
//! sidebar draws and labels it; the document's id is taken from it only
//! when the command is sent.
use super::{HERDR2, NOT_CONNECTED, OTHER_HOST, RenameObject, RenameTarget, WAITING};
use crate::{
    app::Smoke,
    board::snapshot,
    core::{Command, DocRows, DocSpace, TabInfo},
    shell::herdr2_frame::{HERDR2_SESSION, session::split_space_identity},
    terminal_view::TerminalView,
};

/// The synthesized snapshot's ids of the drawn space and tab
/// (`core/screen.rs`), which the palette and the menu bar name.
const DRAWN_SPACE: &str = "herdr-2";
const DRAWN_TAB: &str = "core";

/// Why a Herdr 2 space or tab cannot be renamed: no readable document.
pub(crate) const NO_DOCUMENT: &str = "Can't read the herdr-app layout; Rename needs it";
/// Why an iPad-only tab or another session's space cannot be renamed.
pub(crate) const NOT_IN_DOCUMENT: &str = "Only herdr-app spaces and tabs can be renamed";

/// What the drawn `default` session last read of the document.
#[derive(Clone, Debug)]
pub(crate) struct DocNames {
    pub(crate) version: u64,
    pub(crate) selected: Option<String>,
    pub(crate) spaces: Vec<DocSpace>,
    /// The tabs drawn now (the selected space's, or the iPad-only ones).
    pub(crate) drawn: Vec<TabInfo>,
}

/// The document of the drawn session, when it is `default` and readable.
pub(crate) fn doc_names(terminal: &TerminalView) -> Option<DocNames> {
    if terminal.core.as_ref()?.name() != HERDR2_SESSION {
        return None;
    }
    let row = crate::actions::current_core_session(terminal)?;
    let doc = row.doc?;
    let DocRows::Readable(spaces) = doc.state else {
        return None;
    };
    Some(DocNames {
        version: doc.version,
        selected: doc.selected,
        spaces,
        drawn: row.tabs,
    })
}

/// Why the palette's and the menu bar's `command` is off on Herdr 2, or
/// `None` when it can run.
pub(crate) fn command_unavailable(
    terminal: &TerminalView,
    command: sheprd_commands::Command,
) -> Option<&'static str> {
    use sheprd_commands::Command as C;
    if command == C::RenamePane {
        return Some(HERDR2);
    }
    if !terminal.live.connected || terminal.reconnect.suspended {
        return Some(NOT_CONNECTED);
    }
    let Some(doc) = doc_names(terminal) else {
        return Some(NO_DOCUMENT);
    };
    let held = match command {
        C::RenameWorkspace => doc.selected.is_some(),
        C::RenameTab => doc
            .drawn
            .iter()
            .find(|tab| tab.active)
            .is_some_and(|tab| doc.tab(&tab.id).is_some()),
        _ => false,
    };
    (!held).then_some(NOT_IN_DOCUMENT)
}

impl DocNames {
    fn space(&self, object: &RenameObject) -> Option<&DocSpace> {
        let RenameObject::Space(identity) = object else {
            return None;
        };
        match split_space_identity(identity) {
            (HERDR2_SESSION, Some(id)) => self.spaces.iter().find(|space| space.id == id),
            _ => None,
        }
    }

    fn tab(&self, id: &str) -> Option<(usize, &TabInfo)> {
        self.spaces
            .iter()
            .find_map(|space| space.tabs.iter().enumerate().find(|(_, tab)| tab.id == id))
    }

    fn tab_of(&self, object: &RenameObject) -> Option<(usize, &TabInfo)> {
        match object {
            RenameObject::Tab(id) => self.tab(id),
            _ => None,
        }
    }

    /// Whether the document holds `object` (a pane never counts).
    pub(crate) fn holds(&self, object: &RenameObject) -> bool {
        self.space(object).is_some() || self.tab_of(object).is_some()
    }

    /// The field's first text: the own name, "" for an automatic one.
    pub(crate) fn name(&self, object: &RenameObject) -> String {
        if let Some(space) = self.space(object) {
            return if space.auto_name {
                String::new()
            } else {
                space.name.clone()
            };
        }
        self.tab_of(object)
            .filter(|(_, tab)| !tab.auto_name)
            .map(|(_, tab)| tab.label.clone())
            .unwrap_or_default()
    }

    /// The field's spoken name, as on Herdr 1.
    pub(crate) fn accessibility_label(&self, object: &RenameObject) -> String {
        if let Some(space) = self.space(object) {
            return format!("Rename space {}", space.name);
        }
        let number = self.tab_of(object).map_or(0, |(index, _)| index + 1);
        format!("Rename tab {number}")
    }

    /// The command that writes `name` ("" is the automatic name).
    pub(crate) fn command(&self, object: &RenameObject, name: &str) -> Option<Command> {
        if let Some(space) = self.space(object) {
            return Some(Command::RenameSpace {
                id: space.id.clone(),
                name: name.to_owned(),
            });
        }
        self.tab_of(object).map(|(_, tab)| Command::RenameTab {
            id: tab.id.clone(),
            name: name.to_owned(),
        })
    }
}

impl Smoke {
    /// The document as the drawn session last read it, if it can be renamed.
    pub(crate) fn doc_names(&self) -> Option<DocNames> {
        doc_names(&self.terminal)
    }

    /// The palette's and the menu bar's Herdr 2 target: the synthesized
    /// snapshot's drawn space and tab, named as the document's.
    pub(crate) fn herdr2_object(&self, object: RenameObject) -> RenameObject {
        let Some(doc) = self.doc_names() else {
            return object;
        };
        match &object {
            RenameObject::Space(id) if id == DRAWN_SPACE => match doc.selected {
                Some(space) => RenameObject::Space(format!("{HERDR2_SESSION}\0{space}")),
                None => object,
            },
            RenameObject::Tab(id) if id == DRAWN_TAB => doc
                .drawn
                .iter()
                .find(|tab| tab.active)
                .map_or(object.clone(), |tab| RenameObject::Tab(tab.id.clone())),
            _ => object,
        }
    }

    /// `rename_unavailable` for a Herdr 2 target, after the sheet and Find
    /// checks.
    pub(crate) fn herdr2_rename_unavailable(&self, target: &RenameTarget) -> Option<&'static str> {
        if matches!(target.object, RenameObject::Pane(_)) {
            return Some(HERDR2);
        }
        if target.host != self.terminal.reconnect.target
            || target.generation != self.terminal.link.generation
        {
            return Some(OTHER_HOST);
        }
        let live = &self.terminal.live;
        if !live.connected || self.terminal.reconnect.suspended {
            return Some(NOT_CONNECTED);
        }
        let Some(doc) = self.doc_names() else {
            return Some(NO_DOCUMENT);
        };
        if snapshot(live).is_none_or(|shown| shown.boot_id != target.boot) {
            return Some(target.object.closed_text());
        }
        if !doc.holds(&target.object) {
            return Some(NOT_IN_DOCUMENT);
        }
        if self
            .shell
            .rename
            .pending
            .as_ref()
            .is_some_and(|p| p.boot == target.boot && p.object == target.object && p.sending())
        {
            return Some(WAITING);
        }
        None
    }
}
