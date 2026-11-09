//! Snapshot-only board. Hidden hosts retain their sole event reader and mailbox.
use crate::{app::Smoke, live::Live, terminal_view::TerminalView};
use gpui::{AnyElement, Context, div, prelude::*, rgb};
use sheprd_client::protocol::{AgentStatus, ClientShellSnapshot};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Board {
    pub hidden: BTreeMap<String, TerminalView>,
    pub selection: u64,
    pub roomy: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Space(String),
    Agent(String),
}
pub(crate) struct Navigation {
    pub boot: String,
    pub revision: u64,
    pub target: Target,
    pub request: String,
}
#[derive(Clone)]
pub(crate) struct Row {
    pub label: String,
    pub status: AgentStatus,
    pub target: Target,
}
pub(crate) fn snapshot(live: &Live) -> Option<&ClientShellSnapshot> {
    live.pending_snapshot
        .as_deref()
        .or(live.snapshot.as_deref())
}
pub(crate) fn rows(live: &Live) -> Vec<Row> {
    let Some(snapshot) = snapshot(live) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for space in &snapshot.workspaces {
        rows.push(Row {
            label: format!("Space {} · {}", space.number, space.label),
            status: space.agent_status,
            target: Target::Space(space.workspace_id.clone()),
        });
        for agent in snapshot
            .agents
            .iter()
            .filter(|a| a.workspace_id == space.workspace_id)
        {
            rows.push(agent_row(agent));
        }
    }
    // Do not silently lose an agent if a daemon snapshot omits its workspace.
    for agent in snapshot.agents.iter().filter(|a| {
        !snapshot
            .workspaces
            .iter()
            .any(|w| w.workspace_id == a.workspace_id)
    }) {
        rows.push(agent_row(agent));
    }
    rows
}
fn agent_row(agent: &sheprd_client::protocol::ClientShellAgent) -> Row {
    Row {
        label: format!(
            "  [{}] {}",
            agent
                .display_agent
                .as_deref()
                .or(agent.agent.as_deref())
                .unwrap_or("Unknown harness"),
            agent
                .name
                .as_deref()
                .or(agent.title.as_deref())
                .unwrap_or(&agent.pane_id)
        ),
        status: agent.agent_status,
        target: Target::Agent(agent.pane_id.clone()),
    }
}
fn status(status: AgentStatus) -> (&'static str, u32) {
    match status {
        AgentStatus::Idle => ("Idle", 0x9ca3af),
        AgentStatus::Working => ("Working", 0x88c0d0),
        AgentStatus::Blocked => ("Blocked", 0xebcb8b),
        AgentStatus::Done => ("Done", 0xa3be8c),
        AgentStatus::Unknown => ("Unknown", 0xb48ead),
    }
}
pub(crate) fn finish_navigation(terminal: &mut TerminalView) {
    let Some(nav) = &terminal.navigating else {
        return;
    };
    let confirmed = snapshot(&terminal.live).is_some_and(|s| {
        s.boot_id == nav.boot
            && s.revision >= nav.revision
            && (s.revision > nav.revision
                || terminal.live.pending_request.as_ref() != Some(&nav.request))
            && match &nav.target {
                Target::Space(id) => s.focused_workspace_id.as_ref() == Some(id),
                Target::Agent(id) => s.focused_pane_id.as_ref() == Some(id),
            }
    });
    if confirmed
        || terminal.live.completed_request.as_ref() == Some(&nav.request)
        || !terminal.live.connected
        || terminal.live.failed_request.as_ref() == Some(&nav.request)
    {
        terminal.navigating = None;
    }
}

impl Smoke {
    pub(crate) fn select_host(&mut self, host: &str) {
        if self.terminal.reconnect.target == host {
            self.terminal.open = true;
            return;
        }
        self.terminal.input.detach();
        let next = self.board.hidden.remove(host).unwrap_or_default();
        let mut previous = std::mem::replace(&mut self.terminal, next);
        previous.input.detach();
        if !previous.reconnect.target.is_empty() {
            self.board
                .hidden
                .insert(previous.reconnect.target.clone(), previous);
        }
        self.terminal.reconnect.target = host.to_owned();
        self.terminal.input.detach();
        self.terminal.size = None;
        self.terminal.open = true;
        self.terminal.needs_focus = true;
        self.board.selection = self.board.selection.wrapping_add(1);
    }
    pub(crate) fn record_board(&self, host: &str, live: &Live) {
        self.observation.record(serde_json::json!({"kind":"board", "host":host, "selected":host == self.terminal.reconnect.target, "revision":snapshot(live).map(|s|s.revision), "rows":rows(live).iter().map(|r|r.label.clone()).collect::<Vec<_>>(), "states":rows(live).iter().map(|r|status(r.status).0).collect::<Vec<_>>() }));
    }
    pub(crate) fn poll_hidden(&mut self, cx: &mut Context<Self>) {
        let mut updates = Vec::new();
        for (host, terminal) in &mut self.board.hidden {
            if let Some(mut live) = terminal.link.take_update() {
                // Hidden host clipboard effects are consumed, never replayed on selection.
                live.clipboard.clear();
                terminal.live = live;
                finish_navigation(terminal);
                updates.push(host.clone());
            }
        }
        for host in &updates {
            if let Some(terminal) = self.board.hidden.get(host) {
                self.record_board(host, &terminal.live);
            }
        }
        if !updates.is_empty() {
            cx.notify();
        }
    }
    pub(crate) fn navigate_board(
        &mut self,
        host: &str,
        target: Target,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        self.select_host(host);
        self.terminal.input.detach();
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host state busy; tap again".into()))?;
        if !live.connected || terminal.navigating.is_some() {
            return Err(crate::Error::Ssh(
                "Host unavailable or navigation pending".into(),
            ));
        }
        let s = snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        if s.boot_id != boot || terminal.link.generation != generation {
            return Err(crate::Error::Ssh("Stale board row; tap again".into()));
        }
        if !rows(&live).iter().any(|row| row.target == target) {
            return Err(crate::Error::Ssh("Target no longer exists".into()));
        }
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = match &target {
            Target::Space(id) => handle.focus_workspace(&s.boot_id, id),
            Target::Agent(id) => handle.focus_pane(&s.boot_id, id),
        }
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        self.observation.record(serde_json::json!({"kind":"navigation", "host":host, "boot":s.boot_id, "generation":generation, "revision":s.revision, "pane": match &target { Target::Space(id) => s.panes.iter().find(|p| &p.workspace_id == id).map(|p| p.pane_id.as_str()).unwrap_or_default(), Target::Agent(id) => id.as_str() }}));
        terminal.navigating = Some(Navigation {
            boot: s.boot_id.clone(),
            revision: s.revision,
            target,
            request: request.clone(),
        });
        live.failed_request = None;
        live.completed_request = None;
        live.pending_request = Some(request);
        Ok(())
    }
    pub(crate) fn render_board(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut hosts = self
            .board
            .hidden
            .iter()
            .map(|(host, t)| (host.clone(), &t.live, t.link.generation))
            .collect::<Vec<_>>();
        hosts.push((
            self.terminal.reconnect.target.clone(),
            &self.terminal.live,
            self.terminal.link.generation,
        ));
        hosts.sort_by(|a, b| a.0.cmp(&b.0));
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                div()
                    .id("board-density")
                    .px_4()
                    .py_2()
                    .bg(rgb(0x434c5e))
                    .child(if self.board.roomy {
                        "Board: Roomy"
                    } else {
                        "Board: Dense"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.board.roomy ^= true;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("status-board")
                    .h(gpui::px(if self.board.roomy { 260. } else { 180. }))
                    .overflow_y_scroll()
                    .children(hosts.into_iter().enumerate().map(
                        |(i, (host, live, generation))| {
                            let selected = host == self.terminal.reconnect.target;
                            let choose = host.clone();
                            div()
                                .flex()
                                .flex_col()
                                .px_4()
                                .child(
                                    div()
                                        .id(("board-host", i))
                                        .py_2()
                                        .child(format!(
                                            "{}{} · {}",
                                            if selected { "▸ " } else { "" },
                                            host,
                                            live.status
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_host(&choose);
                                            window.dismiss_virtual_keyboard();
                                            cx.notify();
                                        })),
                                )
                                .children(rows(live).into_iter().enumerate().map(|(j, row)| {
                                    let host = host.clone();
                                    let boot = snapshot(live)
                                        .map(|s| s.boot_id.clone())
                                        .unwrap_or_default();
                                    let (label, color) = status(row.status);
                                    div()
                                        .id(("board-row", i * 65536 + j))
                                        .py(gpui::px(if self.board.roomy { 12. } else { 4. }))
                                        .text_color(rgb(color))
                                        .child(format!("● {} · {}", row.label, label))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            window.dismiss_virtual_keyboard();
                                            if let Err(e) = this.navigate_board(
                                                &host,
                                                row.target.clone(),
                                                &boot,
                                                generation,
                                            ) {
                                                this.terminal.live.status = e.to_string();
                                            }
                                            cx.notify();
                                        }))
                                }))
                        },
                    )),
            )
            .into_any_element()
    }
}
#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
