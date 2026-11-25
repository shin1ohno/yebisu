//! The Herdr 2 source and navigation for the shared sidebar.
use super::session;
use crate::{
    app::Smoke,
    core::SessionInfo,
    shell::{
        HerdrKind,
        sidebar::{
            SidebarState,
            model::{HostEntry, HostLink, host_link, order_hosts},
        },
    },
    terminal_view::TerminalView,
};
use gpui::Context;
use std::collections::BTreeMap;

/// The Herdr 2 spaces a view listed while it was connected, and the one it
/// served. The sidebar draws them, not connected, while the view has no
/// connected session: a host whose only session ended (a lost link, a
/// failed dial, Background, Detach) would otherwise lose every space,
/// `default` included, until a new space made a new session.
#[derive(Clone, Debug, Default)]
pub(crate) struct Listed {
    pub(crate) current: String,
    pub(crate) rows: Vec<SessionInfo>,
}

impl Listed {
    /// The space was removed: it is not drawn again.
    pub(crate) fn forget(&mut self, name: &str) {
        self.rows.retain(|row| row.name != name);
    }

    fn holds(&self, name: &str) -> Option<&SessionInfo> {
        self.rows
            .iter()
            .find(|row| row.name == name && row.confirmed)
    }
}

impl TerminalView {
    /// Keeps the list of a connected session for `Listed`; called with each
    /// update the view takes.
    pub(crate) fn note_listed(&mut self) {
        if !self.core_mode || !self.live.connected {
            return;
        }
        let Some(core) = &self.core else {
            return;
        };
        let rows = core.sessions(self.live.input_epoch);
        if !rows.is_empty() {
            self.listed = Listed {
                current: core.name().to_owned(),
                rows,
            };
        }
    }
}

/// Herdr 2 host rows: every Herdr 2 terminal view (current and hidden)
/// whose session has a list, or that listed spaces before. A host with
/// neither has no row; one that is not connected keeps its row with the
/// spaces it listed last, each not connected.
pub(crate) fn herdr2_hosts(
    state: &SidebarState,
    current: &TerminalView,
    hidden: &BTreeMap<String, TerminalView>,
) -> Vec<HostEntry> {
    let target = current.reconnect.target.as_str();
    let mut hosts = hidden
        .iter()
        .map(|(host, terminal)| (host.as_str(), terminal))
        .chain(std::iter::once((target, current)))
        .filter(|(host, terminal)| !host.is_empty() && terminal.core_mode)
        .filter_map(|(host, terminal)| {
            let link = host_link(&terminal.live);
            let core = terminal.core.as_ref();
            let statuses = core.and_then(crate::core::Session::statuses);
            let catalog =
                core.map_or_else(Vec::new, |core| core.sessions(terminal.live.input_epoch));
            #[cfg(test)]
            let catalog = match core {
                Some(core) if catalog.is_empty() && statuses.is_some() => {
                    session::bootstrap_catalog(core.name())
                }
                _ => catalog,
            };
            let offline = link != HostLink::Connected && !terminal.listed.rows.is_empty();
            if catalog.is_empty() && !offline {
                return None;
            }
            let mut entry = HostEntry::new(host, HerdrKind::Herdr2, link);
            entry.selected = std::ptr::eq(terminal, current);
            entry.generation = terminal.link.generation;
            entry.boot = crate::board::snapshot(&terminal.live)
                .map_or_else(String::new, |snapshot| snapshot.boot_id.clone());
            let mut entry = session::host_entry(
                entry,
                core.map_or(terminal.listed.current.as_str(), |core| core.name()),
                (&catalog, &terminal.listed.rows),
                statuses.as_deref().unwrap_or_default(),
                state,
            );
            for space in &mut entry.spaces {
                space.current &= entry.selected;
            }
            Some(entry)
        })
        .collect::<Vec<_>>();
    order_hosts(&mut hosts, &state.host_order);
    hosts
}

#[derive(Clone)]
pub(crate) enum CoreRow {
    Space(String),
    Agent(String),
}

impl Smoke {
    /// Route a Herdr 2 row from its drawn host and connection generation.
    pub(crate) fn open_herdr2_row(
        &mut self,
        host: &str,
        boot: &str,
        generation: u64,
        target: CoreRow,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        if self.input_modal() || self.new_space_open() {
            return Err(crate::Error::Ssh("Close the sheet first".into()));
        }
        if let CoreRow::Space(identity) = &target
            && let Some(view) = self.terminal_for(host, HerdrKind::Herdr2)
            && view.core_mode
            && !view.live.connected
            && view.link.generation == generation
        {
            return self.open_listed_space(host, identity, cx);
        }
        let view = self
            .terminal_for(host, HerdrKind::Herdr2)
            .filter(|view| {
                view.core_mode
                    && view.live.connected
                    && view.link.generation == generation
                    && crate::board::snapshot(&view.live)
                        .map_or("", |snapshot| snapshot.boot_id.as_str())
                        == boot
            })
            .ok_or_else(|| crate::Error::Ssh("Stale Herdr 2 row; tap again".into()))?;
        let core = view
            .core
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Stale Herdr 2 row; tap again".into()))?;
        let (session, address, space) = match &target {
            CoreRow::Space(identity) => {
                let (session, space) = session::split_space_identity(identity);
                (session, None, space)
            }
            CoreRow::Agent(identity) => {
                let (session, address) = session::split_pane_identity(identity);
                (session, Some(address), None)
            }
        };
        // #144 D2: a document space row draws that space; the plain
        // `default` row of a host reading the document draws the iPad-only
        // tabs (owner answer D2-1).
        let documented = core
            .sessions(view.live.input_epoch)
            .iter()
            .any(|row| row.name == super::HERDR2_SESSION && row.doc.is_some());
        let select_space = (address.is_none()
            && session == super::HERDR2_SESSION
            && (space.is_some() || documented))
            .then(|| crate::core::Command::SelectDocSpace(space.map(str::to_owned)));
        if session == core.name() {
            self.select_host_kind(host, HerdrKind::Herdr2);
            self.shell.sidebar.reveal_host(host);
            if let Some(address) = address {
                self.try_core_command(crate::core::Command::FocusAddress(address.to_owned()))?;
            }
            if let Some(command) = select_space {
                self.try_core_command(command)?;
            }
            return Ok(());
        }
        let expected_core_id = {
            let sessions = core.sessions(view.live.input_epoch);
            let row = sessions.iter().find(|row| row.name == session);
            let Some(row) = row else {
                return Err(crate::Error::Ssh("Stale Herdr 2 session; tap again".into()));
            };
            if !row.confirmed {
                return Err(crate::Error::Ssh("Herdr 2 session is unconfirmed".into()));
            }
            if let Some(address) = address
                && !row.statuses.iter().any(|status| status.address == address)
            {
                return Err(crate::Error::Ssh("Stale Herdr 2 agent; tap again".into()));
            }
            row.core_id.clone()
        };
        self.connect_core_target(
            crate::core::SessionTarget {
                host: host.to_owned(),
                name: session.to_owned(),
                focus: address.map(str::to_owned),
                expected_core_id,
            },
            cx,
        )?;
        // The new session's queue takes commands before its worker starts.
        match select_space {
            Some(command) => self.try_core_command(command),
            None => Ok(()),
        }
    }

    /// A space row drawn while its host had no connected session (`Listed`):
    /// the tap dials that space, as its row's Reconnect would.
    fn open_listed_space(
        &mut self,
        host: &str,
        identity: &str,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let (session, _) = session::split_space_identity(identity);
        let row = self
            .terminal_for(host, HerdrKind::Herdr2)
            .and_then(|view| view.listed.holds(session))
            .ok_or_else(|| crate::Error::Ssh("Stale Herdr 2 row; tap again".into()))?;
        let target = crate::core::SessionTarget {
            host: host.to_owned(),
            name: session.to_owned(),
            focus: None,
            expected_core_id: row.core_id.clone(),
        };
        self.shell.sidebar.reveal_host(host);
        self.connect_core_target(target, cx)
    }
}
