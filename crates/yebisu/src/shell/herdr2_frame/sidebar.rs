//! The Herdr 2 source and navigation for the shared sidebar.
use super::session;
use crate::{
    app::Smoke,
    shell::{
        HerdrKind,
        sidebar::{
            SidebarState,
            model::{HostEntry, host_link, order_hosts},
        },
    },
    terminal_view::TerminalView,
};
use gpui::Context;
use std::collections::BTreeMap;

/// Herdr 2 host rows: every Herdr 2 terminal view (current and hidden)
/// whose session has a list. A host with no session or no list yet has no
/// row; one that is not connected keeps its row without its session.
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
            let core = terminal.core.as_ref()?;
            let statuses = core.statuses();
            let catalog = core.sessions(terminal.live.input_epoch);
            #[cfg(test)]
            let catalog = if catalog.is_empty() && statuses.is_some() {
                session::bootstrap_catalog(core.name())
            } else {
                catalog
            };
            if catalog.is_empty() {
                return None;
            }
            let mut entry = HostEntry::new(host, HerdrKind::Herdr2, host_link(&terminal.live));
            entry.selected = std::ptr::eq(terminal, current);
            entry.generation = terminal.link.generation;
            entry.boot = crate::board::snapshot(&terminal.live)
                .map_or_else(String::new, |snapshot| snapshot.boot_id.clone());
            let mut entry = session::host_entry(
                entry,
                core.name(),
                &catalog,
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
        let (session, address) = match &target {
            CoreRow::Space(session) => (session.as_str(), None),
            CoreRow::Agent(identity) => {
                let (session, address) = session::split_pane_identity(identity);
                (session, Some(address))
            }
        };
        if session == core.name() {
            self.select_host_kind(host, HerdrKind::Herdr2);
            self.shell.sidebar.reveal_host(host);
            if let Some(address) = address {
                self.try_core_command(crate::core::Command::FocusAddress(address.to_owned()))?;
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
        )
    }
}
