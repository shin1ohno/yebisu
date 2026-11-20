//! The legacy Herdr 2 sidebar and the source of Herdr 2 host entries.
use super::session;
use crate::{
    app::Smoke,
    hosts::HostName,
    shell::sidebar::{
        AGENT_ROW_HEIGHT, HOST_ROW_HEIGHT, SPACE_ROW_HEIGHT, SidebarState,
        model::{
            AgentLine, HostEntry, HostLink, OfflineReason, agents_list, host_link, order_hosts,
        },
        row_height,
        view::{AGENTS_HEADING, HOSTS_HEADING, SidebarTap},
    },
    shell::style,
    terminal_view::TerminalView,
};
use gpui::{AnyElement, Context, div, prelude::*, px, rgb};
use std::collections::BTreeMap;

/// The list's element id, for the frame's tests.
pub(crate) const HERDR2_LIST_ID: &str = "herdr2-sidebar-list";
const MUTED: u32 = style::color::MUTED;

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
            let mut entry =
                HostEntry::new(host, super::HerdrKind::Herdr2, host_link(&terminal.live));
            entry.selected = host == target;
            entry.generation = terminal.link.generation;
            entry.boot = crate::board::snapshot(&terminal.live)
                .map_or_else(String::new, |snapshot| snapshot.boot_id.clone());
            Some(session::host_entry(
                entry,
                core.name(),
                &catalog,
                statuses.as_deref().unwrap_or_default(),
                state,
            ))
        })
        .collect::<Vec<_>>();
    order_hosts(&mut hosts, &state.host_order);
    hosts
}

fn heading(text: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .text_size(px(style::text::SECONDARY))
        .text_color(rgb(MUTED))
        .child(text)
}

fn second_line(text: String) -> gpui::Div {
    div()
        .text_size(px(style::text::SECONDARY))
        .text_color(rgb(MUTED))
        .min_w_0()
        .truncate()
        .child(text)
}

fn host_dot(link: HostLink) -> (&'static str, u32) {
    match link {
        HostLink::Connected => ("●", style::color::GREEN),
        HostLink::Connecting => ("◌", style::color::YELLOW),
        HostLink::Offline(OfflineReason::NotConnected) => ("○", MUTED),
        HostLink::Offline(_) => ("●", style::color::RED),
    }
}

/// One row's two lines: a mark, the name and a trailing note, then detail.
fn row(
    id: (&'static str, usize),
    height: f32,
    mark: (&'static str, u32),
    name: String,
    note: Option<(String, u32)>,
    detail: String,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || format!("{}-{}", id.0, id.1))
        .h(px(row_height(height, 1.0)))
        .flex()
        .flex_col()
        .justify_center()
        .px_3()
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .min_w_0()
                .child(div().text_color(rgb(mark.1)).child(mark.0))
                .child(div().flex_1().min_w_0().truncate().child(name))
                .children(note.map(|(note, color)| {
                    div()
                        .flex_shrink_0()
                        .text_size(px(style::text::SECONDARY))
                        .text_color(rgb(color))
                        .child(note)
                })),
        )
        .child(second_line(detail))
}

impl Smoke {
    /// The Herdr 2 list for the sidebar column, or `None` when no Herdr 2
    /// host has a session list.
    pub(crate) fn render_herdr2_sidebar_list(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let hosts = herdr2_hosts(&self.shell.sidebar, &self.terminal, &self.board.hidden);
        if hosts.is_empty() {
            return None;
        }
        let agents = agents_list(&hosts);
        let mut list = div()
            .id(HERDR2_LIST_ID)
            .debug_selector(|| HERDR2_LIST_ID.into())
            .flex()
            .flex_col()
            .w_full()
            .child(heading(HOSTS_HEADING));
        for (i, host) in hosts.iter().enumerate() {
            let (dot, color) = host_dot(host.link);
            let selected = host.selected;
            list = list.child(
                self.herdr2_tap(
                    row(
                        ("herdr2-host", i),
                        HOST_ROW_HEIGHT,
                        (dot, color),
                        HostName::new(&host.host).to_string(),
                        None,
                        host.detail().to_owned(),
                    )
                    .when(selected, |d| {
                        d.bg(gpui::Rgba::from(style::tint::CURRENT_ROW))
                    }),
                    &host.host,
                    cx,
                ),
            );
            for (j, space) in host.spaces.iter().enumerate() {
                let tabs = (
                    format!(
                        "{} {}",
                        space.tabs,
                        if space.tabs == 1 { "tab" } else { "tabs" }
                    ),
                    MUTED,
                );
                let navigation = CoreRow::Space(space.id.clone());
                let host_id = host.host.clone();
                let boot = host.boot.clone();
                let generation = host.generation;
                list = list.child(
                    self.herdr2_row_tap(
                        row(
                            ("herdr2-space", i * 4096 + j),
                            SPACE_ROW_HEIGHT,
                            ("●", space.dot.dot_color()),
                            space.name.clone(),
                            Some(tabs),
                            space.detail.clone(),
                        )
                        .pl_6()
                        .when(space.current, |d| {
                            d.bg(gpui::Rgba::from(style::tint::CURRENT_ROW))
                        }),
                        host_id,
                        boot,
                        generation,
                        navigation,
                        cx,
                    ),
                );
            }
        }
        if !agents.is_empty() {
            list = list.child(heading(AGENTS_HEADING));
            for (k, line) in agents.iter().enumerate() {
                if let Some(host) = hosts
                    .iter()
                    .find(|host| host.host == line.row.identity.host)
                {
                    list =
                        list.child(self.herdr2_agent_row(k, line, &host.boot, host.generation, cx));
                }
            }
        }
        Some(list.into_any_element())
    }

    fn herdr2_agent_row(
        &self,
        k: usize,
        line: &AgentLine,
        boot: &str,
        generation: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let status = line.row.status;
        self.herdr2_row_tap(
            row(
                ("herdr2-agent", k),
                AGENT_ROW_HEIGHT,
                ("●", status.dot_color()),
                line.shown.clone(),
                Some((status.label().to_owned(), status.label_color())),
                line.place.clone(),
            ),
            line.row.identity.host.clone(),
            boot.to_owned(),
            generation,
            CoreRow::Agent(line.row.identity.pane.clone()),
            cx,
        )
    }

    /// A host row retains the shared host selection behavior.
    fn herdr2_tap(
        &self,
        row: gpui::Stateful<gpui::Div>,
        host: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tap = SidebarTap::Host(host.to_owned());
        row.on_click(cx.listener(move |this, _, window, cx| {
            window.dismiss_virtual_keyboard();
            if let Err(e) = this.sidebar_tap(tap.clone()) {
                this.terminal.live.status = e.to_string();
            }
            cx.notify();
        }))
        .into_any_element()
    }
}

#[derive(Clone)]
pub(crate) enum CoreRow {
    Space(String),
    Agent(String),
}

impl Smoke {
    fn herdr2_row_tap(
        &self,
        row: gpui::Stateful<gpui::Div>,
        host: String,
        boot: String,
        generation: u64,
        target: CoreRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        row.on_click(cx.listener(move |this, _, window, cx| {
            window.dismiss_virtual_keyboard();
            if let Err(error) = this.open_herdr2_row(&host, &boot, generation, target.clone(), cx) {
                this.terminal.live.status = error.to_string();
            }
            cx.notify();
        }))
        .into_any_element()
    }

    /// Route a Herdr 2 row from its drawn host and connection generation.
    /// A2 uses this for Herdr 2 branches of the merged space and AGENTS lists.
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
        let current = self.terminal.reconnect.target == host;
        let view = if current {
            Some(&self.terminal)
        } else {
            self.board.hidden.get(host)
        };
        let view = view
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
        if current && session == core.name() {
            if let Some(address) = address {
                self.try_core_command(crate::core::Command::FocusAddress(address.to_owned()))?;
            }
            return Ok(());
        }
        let expected_core_id = if session != core.name() {
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
        } else {
            None
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
