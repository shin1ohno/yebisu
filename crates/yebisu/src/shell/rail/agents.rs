use super::{RailItem, RailItemKind, RailTarget};
use crate::{
    app::Smoke,
    board::snapshot,
    shell::{
        HerdrKind,
        herdr2_frame::{HERDR2_SESSION, sidebar::herdr2_hosts},
        sidebar::{
            model::{HostEntry, order_hosts},
            view::host_entries,
        },
        sidebar_list::sources,
    },
    terminal_view::TerminalView,
};

// Match the sidebar catalog's session-qualified addresses, including its legacy
// bare addresses for the default session.
pub(super) fn core_agent_session(pane: &str) -> &str {
    pane.split_once('\0')
        .map_or(HERDR2_SESSION, |(session, _)| session)
}

impl Smoke {
    pub(super) fn rail_core_hosts(&self) -> Vec<HostEntry> {
        herdr2_hosts(&self.shell.sidebar, &self.terminal, &self.board.hidden_core)
    }

    pub(super) fn rail_terminal(&self, host: &str) -> Option<&TerminalView> {
        self.board
            .terminals(&self.terminal)
            .find(|terminal| terminal.reconnect.target == host)
    }

    pub(super) fn rail_agent_items(&self) -> Vec<RailItem> {
        let state = &self.shell.sidebar;
        let mut hosts = sources(
            host_entries(
                state,
                &self.terminal,
                self.board.hidden.iter().chain(&self.board.hidden_core),
            ),
            self.rail_core_hosts(),
        );
        if state.host_order.is_empty() {
            order_hosts(&mut hosts, &self.setup.hosts);
        }
        state
            .ordered_agents(&hosts)
            .into_iter()
            .filter_map(|line| {
                let id = line.row.identity;
                let host = hosts
                    .iter()
                    .find(|host| host.host == id.host && host.kind == id.kind)?;
                let workspace_id = match id.kind {
                    HerdrKind::Herdr1 => {
                        let terminal = self.terminal_for(&id.host, id.kind)?;
                        let shown = snapshot(&terminal.live)?;
                        shown
                            .panes
                            .iter()
                            .find(|pane| pane.pane_id == id.pane)?
                            .workspace_id
                            .as_str()
                    }
                    HerdrKind::Herdr2 => core_agent_session(&id.pane),
                };
                let space = host.spaces.iter().find(|space| space.id == workspace_id)?;
                let workspace_id = workspace_id.to_owned();
                Some(RailItem {
                    kind: RailItemKind::Agent,
                    name: line.shown,
                    host: self.setup.host_name(&id.host).to_string(),
                    space: Some(space.name.clone()),
                    herdr: id.kind,
                    status: Some(line.row.status),
                    connected: true,
                    current: false,
                    pinned: state.is_agent_pinned(&id, &host.boot),
                    needs: 0,
                    error: 0,
                    boot: host.boot.clone(),
                    generation: host.generation,
                    icon: Some(line.row.icon),
                    target: RailTarget::Agent {
                        id,
                        workspace_id,
                        boot: host.boot.clone(),
                        generation: host.generation,
                    },
                })
            })
            .collect()
    }
}
