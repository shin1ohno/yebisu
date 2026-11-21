use super::{RailItem, RailItemKind, RailTarget};
use crate::{
    app::Smoke,
    board::snapshot,
    hosts::HostName,
    shell::sidebar::{model::order_hosts, view::host_entries},
    terminal_view::TerminalView,
};

impl Smoke {
    pub(super) fn rail_terminal(&self, host: &str) -> Option<&TerminalView> {
        if host == self.terminal.reconnect.target {
            Some(&self.terminal)
        } else {
            self.board.hidden.get(host)
        }
    }

    pub(super) fn rail_agent_items(&self) -> Vec<RailItem> {
        let state = &self.shell.sidebar;
        let mut hosts = host_entries(state, &self.terminal, &self.board.hidden);
        if state.host_order.is_empty() {
            order_hosts(&mut hosts, &self.setup.hosts);
        }
        state
            .ordered_agents(&hosts)
            .into_iter()
            .filter_map(|line| {
                let id = line.row.identity;
                let terminal = self.rail_terminal(&id.host)?;
                let shown = snapshot(&terminal.live)?;
                let pane = shown.panes.iter().find(|pane| pane.pane_id == id.pane)?;
                Some(RailItem {
                    kind: RailItemKind::Agent,
                    name: line.shown,
                    host: HostName::new(&id.host).to_string(),
                    herdr: id.kind,
                    status: Some(line.row.status),
                    connected: true,
                    current: false,
                    pinned: state.is_agent_pinned(&id, &shown.boot_id),
                    needs: 0,
                    error: 0,
                    boot: shown.boot_id.clone(),
                    generation: terminal.link.generation,
                    target: RailTarget::Agent {
                        id,
                        workspace_id: pane.workspace_id.clone(),
                        boot: shown.boot_id.clone(),
                        generation: terminal.link.generation,
                    },
                })
            })
            .collect()
    }
}
