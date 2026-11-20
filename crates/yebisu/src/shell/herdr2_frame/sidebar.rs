//! L8: Herdr 2 sessions in the sidebar column (plan v2 7.14). Owner: L8.
//!
//! L3's list (`sidebar::view`) draws Herdr 1. On Herdr 2 the frame's column
//! keeps the L2b footer and draws this list above it: each Herdr 2 host the
//! app holds whose session has listed its terminals (`core::status`), with
//! its one session row and the agents that reported a status. With no such
//! host there is no list, so the column shows only the footer
//! (`herdr2_without_a_sidebar_list_keeps_the_footer_column`). Herdr 2 has
//! no board navigation: every row selects its host.
use super::session;
use crate::{
    app::Smoke,
    shell::sidebar::{
        AGENT_ROW_HEIGHT, HOST_ROW_HEIGHT, SPACE_ROW_HEIGHT, SidebarState,
        model::{
            AgentLine, HostEntry, HostLink, OfflineReason, agents_list, host_link, order_hosts,
        },
        row_height,
        view::{AGENTS_HEADING, HOSTS_HEADING, SidebarTap},
    },
    terminal_view::TerminalView,
};
use gpui::{AnyElement, Context, div, prelude::*, px, rgb};
use std::collections::BTreeMap;

/// The list's element id, for the frame's tests.
pub(crate) const HERDR2_LIST_ID: &str = "herdr2-sidebar-list";
const MUTED: u32 = 0x9ca3af;
const SELECTED: u32 = 0x3b4252;

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
            let statuses = terminal.core.as_ref()?.statuses()?;
            let mut entry = session::host_entry(
                host,
                host_link(&terminal.live),
                terminal.link.generation,
                &statuses,
                state,
            );
            entry.selected = host == target;
            Some(entry)
        })
        .collect::<Vec<_>>();
    order_hosts(&mut hosts, &state.host_order);
    hosts
}

fn heading(text: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .text_sm()
        .text_color(rgb(MUTED))
        .child(text)
}

fn second_line(text: String) -> gpui::Div {
    div()
        .text_sm()
        .text_color(rgb(MUTED))
        .min_w_0()
        .truncate()
        .child(text)
}

fn host_dot(link: HostLink) -> (&'static str, u32) {
    match link {
        HostLink::Connected => ("●", 0xa3be8c),
        HostLink::Connecting => ("◌", 0xebcb8b),
        HostLink::Offline(OfflineReason::NotConnected) => ("○", MUTED),
        HostLink::Offline(_) => ("●", 0xbf616a),
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
                        .text_sm()
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
                        host.host.clone(),
                        None,
                        host.detail().to_owned(),
                    )
                    .when(selected, |d| d.bg(rgb(SELECTED))),
                    &host.host,
                    cx,
                ),
            );
            for (j, space) in host.spaces.iter().enumerate() {
                let tabs = (format!("{} tab", space.tabs), MUTED);
                list = list.child(
                    self.herdr2_tap(
                        row(
                            ("herdr2-space", i * 4096 + j),
                            SPACE_ROW_HEIGHT,
                            ("●", space.dot.dot_color()),
                            space.name.clone(),
                            Some(tabs),
                            space.detail.clone(),
                        )
                        .pl_6()
                        .when(space.current, |d| d.bg(rgb(SELECTED))),
                        &host.host,
                        cx,
                    ),
                );
            }
        }
        if !agents.is_empty() {
            list = list.child(heading(AGENTS_HEADING));
            for (k, line) in agents.iter().enumerate() {
                list = list.child(self.herdr2_agent_row(k, line, cx));
            }
        }
        Some(list.into_any_element())
    }

    fn herdr2_agent_row(&self, k: usize, line: &AgentLine, cx: &mut Context<Self>) -> AnyElement {
        let status = line.row.status;
        self.herdr2_tap(
            row(
                ("herdr2-agent", k),
                AGENT_ROW_HEIGHT,
                ("●", status.dot_color()),
                line.shown.clone(),
                Some((status.label().to_owned(), status.label_color())),
                line.place.clone(),
            ),
            &line.row.identity.host,
            cx,
        )
    }

    /// Every Herdr 2 row selects its host, as L3's host row does.
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
