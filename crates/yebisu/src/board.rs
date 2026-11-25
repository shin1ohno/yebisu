//! Snapshot-only board. Hidden hosts retain their sole event reader and mailbox.
use crate::{app::Smoke, live::Live, shell::HerdrKind, terminal_view::TerminalView};
use gpui::{AnyElement, Context, div, prelude::*, rgb};
use sheprd_client::protocol::{AgentStatus, ClientShellSnapshot};
use std::collections::BTreeMap;

/// The board keeps one height; the Dense / Roomy toggle is gone (q7).
pub(crate) const BOARD_HEIGHT: f32 = 180.;
#[derive(Default)]
pub(crate) struct Board {
    pub hidden: BTreeMap<String, TerminalView>,
    pub hidden_core: BTreeMap<String, TerminalView>,
    pub selection: u64,
}
impl Board {
    /// App-wide view traversal starts with the selected terminal.
    pub(crate) fn terminals<'a>(
        &'a self,
        selected: &'a TerminalView,
    ) -> impl Iterator<Item = &'a TerminalView> {
        std::iter::once(selected)
            .chain(self.hidden.values())
            .chain(self.hidden_core.values())
    }

    /// Mutable form of `terminals`, in the same order.
    pub(crate) fn terminals_mut<'a>(
        &'a mut self,
        selected: &'a mut TerminalView,
    ) -> impl Iterator<Item = &'a mut TerminalView> {
        std::iter::once(selected)
            .chain(self.hidden.values_mut())
            .chain(self.hidden_core.values_mut())
    }
}

impl TerminalView {
    pub(crate) fn herdr_kind(&self) -> HerdrKind {
        if self.core_mode {
            HerdrKind::Herdr2
        } else {
            HerdrKind::Herdr1
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Space(String),
    Agent(String),
    Command,
}
#[derive(Clone)]
pub(crate) struct Navigation {
    pub boot: String,
    pub revision: u64,
    pub target: Target,
    pub request: String,
}
impl Navigation {
    pub fn matches(&self, s: &ClientShellSnapshot) -> bool {
        s.boot_id == self.boot
            && s.revision >= self.revision
            && match &self.target {
                Target::Space(id) => s.focused_workspace_id.as_ref() == Some(id),
                Target::Agent(id) => s.focused_pane_id.as_ref() == Some(id),
                Target::Command => false,
            }
    }
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
    let coherent = terminal
        .live
        .snapshot
        .as_ref()
        .zip(terminal.live.surface.as_ref())
        .filter(|(s, f)| crate::live::coherent(s, f) && s.boot_id == nav.boot);
    let confirmed = coherent.is_some_and(|(s, _)| {
        let observed = terminal
            .live
            .observed_navigation
            .as_ref()
            .filter(|(request, _)| request == &nav.request)
            .map(|(_, revision)| *revision);
        let current = nav.matches(s).then_some(s.revision);
        observed.or(current).is_some_and(|revision| {
            s.revision >= revision
                && (revision > nav.revision
                    || terminal.live.completed_request.as_ref() == Some(&nav.request))
        })
    });
    // P7 commands have no board target; retain their correlated receipt contract.
    let command_ack = nav.target == Target::Command
        && terminal.live.completed_request.as_ref() == Some(&nav.request);
    if confirmed
        || command_ack
        || !terminal.live.connected
        || terminal.live.failed_request.as_ref() == Some(&nav.request)
    {
        terminal.navigating = None;
    }
}

impl Smoke {
    pub(crate) fn select_host(&mut self, host: &str) {
        self.input_transition(|this| this.select_host_inner(host))
    }
    pub(crate) fn select_host_inner(&mut self, host: &str) {
        let kind = if self.terminal.reconnect.target == host {
            self.terminal.herdr_kind()
        } else if !self.board.hidden.contains_key(host) && self.board.hidden_core.contains_key(host)
        {
            HerdrKind::Herdr2
        } else {
            HerdrKind::Herdr1
        };
        self.select_host_kind_inner(host, kind);
    }

    pub(crate) fn select_host_kind(&mut self, host: &str, kind: HerdrKind) {
        self.input_transition(|this| this.select_host_kind_inner(host, kind))
    }

    pub(crate) fn terminal_for(&self, host: &str, kind: HerdrKind) -> Option<&TerminalView> {
        if self.terminal.reconnect.target == host && self.terminal.herdr_kind() == kind {
            Some(&self.terminal)
        } else {
            match kind {
                HerdrKind::Herdr1 => self.board.hidden.get(host),
                HerdrKind::Herdr2 => self.board.hidden_core.get(host),
            }
        }
    }

    pub(crate) fn herdr1_terminal_mut(&mut self, host: &str) -> Option<&mut TerminalView> {
        if self.terminal.reconnect.target == host && !self.terminal.core_mode {
            Some(&mut self.terminal)
        } else {
            self.board.hidden.get_mut(host)
        }
    }

    fn select_host_kind_inner(&mut self, host: &str, kind: HerdrKind) {
        if !self.terminal.open {
            let marked = if self.smoke_mode {
                self.input.marked.is_some()
            } else {
                self.setup_input.marked.is_some()
            };
            self.terminal.input.retain_discard(marked);
        }
        self.handoff.cancel();
        self.handoff.selection = None;
        if self.terminal.reconnect.target == host && self.terminal.herdr_kind() == kind {
            self.terminal.open = true;
            return;
        }
        if self.terminal.reconnect.target != host {
            // Keep a core reader only while staying on the same destination.
            // Leaving the destination must still stop its worker and tunnel.
            let previous_core = if self.terminal.core_mode {
                Some(&mut self.terminal)
            } else {
                self.board
                    .hidden_core
                    .get_mut(&self.terminal.reconnect.target)
            };
            if let Some(previous) = previous_core {
                previous.reconnect.core_session_on_return = None;
                previous.core.take();
                previous.link.reset();
                previous.live = Live::default();
            }
        } else if let Some(core) = &self.terminal.core {
            core.invalidate_input();
        }
        self.terminal.input.detach();
        let discarded = self.terminal.input.requires_gesture();
        let mut next = match kind {
            HerdrKind::Herdr1 => self.board.hidden.remove(host),
            HerdrKind::Herdr2 => self.board.hidden_core.remove(host),
        }
        .unwrap_or_default();
        next.core_mode = kind == HerdrKind::Herdr2;
        next.input.retain_discard(discarded);
        if self.settings.is_some() {
            next.compact = self.terminal.compact;
            next.font = self.terminal.font.clone();
            // SH1-157 L1: app-wide Input options and Return (settings_view.rs).
            next.input.options.option_alt = self.terminal.input.options.option_alt;
            next.input.options.command_control = self.terminal.input.options.command_control;
            next.input.options.caret_anchor = self.terminal.input.options.caret_anchor;
            next.reconnect.automatic = self.terminal.reconnect.automatic;
        }
        let mut previous = std::mem::replace(&mut self.terminal, next);
        previous.input.detach();
        if !previous.reconnect.target.is_empty() {
            let hidden = if previous.core_mode {
                &mut self.board.hidden_core
            } else {
                &mut self.board.hidden
            };
            hidden.insert(previous.reconnect.target.clone(), previous);
        }
        self.terminal.reconnect.target = host.to_owned();
        self.terminal.input.detach();
        self.terminal.size = None;
        self.terminal.open = true;
        self.terminal.needs_focus = true;
        self.board.selection = self.board.selection.wrapping_add(1);
    }
    pub(crate) fn record_board(&self, host: &str, live: &Live) {
        // Every reader update reaches here, drawn or not: the sidebar's
        // AGENTS receive order is recorded now, not when the list renders.
        self.record_board_connection(host, self.terminal.herdr_kind(), live);
    }

    fn record_board_connection(&self, host: &str, kind: HerdrKind, live: &Live) {
        crate::shell::sidebar::model::record_live(&self.shell.sidebar, host, kind, live);
        self.observation.record(serde_json::json!({"kind":"board", "host":host, "selected":host == self.terminal.reconnect.target && kind == self.terminal.herdr_kind(), "revision":snapshot(live).map(|s|s.revision), "rows":rows(live).iter().map(|r|r.label.clone()).collect::<Vec<_>>(), "states":rows(live).iter().map(|r|status(r.status).0).collect::<Vec<_>>() }));
    }
    pub(crate) fn poll_hidden(&mut self, cx: &mut Context<Self>) {
        let mut updates = Vec::new();
        for (host, terminal) in self
            .board
            .hidden
            .iter_mut()
            .chain(&mut self.board.hidden_core)
        {
            if terminal.core_mode && terminal.core.is_none() {
                continue;
            }
            if let Some(mut live) = terminal.link.take_update() {
                // Hidden host clipboard effects are consumed, never replayed on selection.
                live.clipboard.clear();
                terminal.live = live;
                terminal.note_listed();
                finish_navigation(terminal);
                updates.push((host.clone(), terminal.herdr_kind()));
            }
        }
        for (host, kind) in &updates {
            if let Some(terminal) = self.terminal_for(host, *kind) {
                self.record_board_connection(host, *kind, &terminal.live);
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
        self.input_transition(|this| this.navigate_board_inner(host, target, boot, generation))
    }
    pub(crate) fn navigate_board_inner(
        &mut self,
        host: &str,
        target: Target,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        if self.terminal_for(host, HerdrKind::Herdr1).is_none() {
            return Err(crate::Error::Ssh("Stale board row; tap again".into()));
        }
        self.select_host_kind_inner(host, HerdrKind::Herdr1);
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
            Target::Command => return Err(crate::Error::Ssh("Not a board target".into())),
        }
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        self.observation.record(serde_json::json!({"kind":"navigation", "host":host, "boot":s.boot_id, "generation":generation, "revision":s.revision, "pane": match &target { Target::Space(id) => s.panes.iter().find(|p| &p.workspace_id == id).map(|p| p.pane_id.as_str()).unwrap_or_default(), Target::Agent(id) => id.as_str(), Target::Command => "" }}));
        terminal.navigating = Some(Navigation {
            boot: s.boot_id.clone(),
            revision: s.revision,
            target,
            request: request.clone(),
        });
        live.begin_navigation(terminal.navigating.clone());
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
                    .px_4()
                    .py_2()
                    .bg(rgb(0x434c5e))
                    .child(crate::shell::sidebar::view::HOSTS_HEADING),
            )
            .child(
                div()
                    .id("status-board")
                    .h(gpui::px(BOARD_HEIGHT))
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
                                        .py(gpui::px(4.))
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
