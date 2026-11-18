//! L3: draws the sidebar list and routes its taps. Owner: L3.
use super::{
    AGENT_ROW_HEIGHT, HOST_ROW_HEIGHT, SPACE_ROW_HEIGHT, SidebarState,
    long_press::{PressedRow, long_press_layer},
    model::{
        AgentLine, HostEntry, HostLink, OfflineReason, SpaceEntry, agents_list, host_link,
        order_hosts, record_live, snapshot_content,
    },
    row_height,
};
use crate::{
    app::Smoke,
    board::{Target, snapshot},
    shell::HerdrKind,
    terminal_view::TerminalView,
};
use gpui::{AnyElement, Context, div, prelude::*, px, rgb};
use std::collections::BTreeMap;

/// The list's heading; also the Simulator's scroll anchor (plan v2 5.4).
pub(crate) const HOSTS_HEADING: &str = "HOSTS";
/// Heading above the cross-host agent list.
pub(crate) const AGENTS_HEADING: &str = "AGENTS";
/// Text scale until the OS text size reaches yebisu (plan v2 7.13).
const TEXT_SCALE: f32 = 1.0;
const MUTED: u32 = 0x9ca3af;

/// What a sidebar row does when tapped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SidebarTap {
    Host(String),
    Space {
        host: String,
        id: String,
        boot: String,
        generation: u64,
    },
    Agent {
        host: String,
        pane: String,
        boot: String,
        generation: u64,
    },
}

/// Host rows for every reader this app holds, in the saved Hosts order.
/// The receive order is recorded when updates arrive (`record_live`); this
/// records again only to cover a reader no update has reached yet.
pub(crate) fn host_entries(
    state: &SidebarState,
    current: &TerminalView,
    hidden: &BTreeMap<String, TerminalView>,
) -> Vec<HostEntry> {
    let mut hosts = Vec::new();
    for (host, terminal) in hidden
        .iter()
        .map(|(h, t)| (h.as_str(), t))
        .chain(std::iter::once((
            current.reconnect.target.as_str(),
            current,
        )))
    {
        if host.is_empty() {
            continue;
        }
        let kind = if terminal.core_mode {
            HerdrKind::Herdr2
        } else {
            HerdrKind::Herdr1
        };
        let live = &terminal.live;
        let link = host_link(live);
        let mut entry = HostEntry::new(host, kind, link);
        entry.selected = host == current.reconnect.target;
        entry.generation = terminal.link.generation;
        record_live(state, host, kind, live);
        if let (HostLink::Connected, HerdrKind::Herdr1, Some(snap)) = (link, kind, snapshot(live)) {
            entry.boot = snap.boot_id.clone();
            let (spaces, agents) = snapshot_content(host, snap, state);
            entry = entry.with_content(spaces, agents);
        }
        hosts.push(entry);
    }
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

impl Smoke {
    /// Taps go through PR 40's `input_transition`, like the board's.
    pub(crate) fn sidebar_tap(&mut self, tap: SidebarTap) -> crate::Result<()> {
        self.input_transition(|this| match tap {
            SidebarTap::Host(host) => {
                this.select_host_inner(&host);
                Ok(())
            }
            SidebarTap::Space {
                host,
                id,
                boot,
                generation,
            } => this.navigate_board_inner(&host, Target::Space(id), &boot, generation),
            SidebarTap::Agent {
                host,
                pane,
                boot,
                generation,
            } => this.navigate_board_inner(&host, Target::Agent(pane), &boot, generation),
        })
    }

    fn tap_row(&mut self, tap: SidebarTap, window: &mut gpui::Window, cx: &mut Context<Self>) {
        window.dismiss_virtual_keyboard();
        if let Err(e) = self.sidebar_tap(tap) {
            self.terminal.live.status = e.to_string();
        }
        cx.notify();
    }

    /// The sidebar list; `None` keeps the legacy board in the column.
    /// Herdr 2 keeps `None` (L2a's frame leaves the column empty) until L8
    /// puts its sessions in this list (review SF-6).
    pub(crate) fn render_sidebar_list(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.terminal.core_mode {
            return None;
        }
        let hosts = host_entries(&self.shell.sidebar, &self.terminal, &self.board.hidden);
        let agents = agents_list(&hosts);
        let mut list = div()
            .id("sidebar-list")
            .flex()
            .flex_col()
            .w_full()
            .overflow_y_scroll()
            .child(heading(HOSTS_HEADING));
        for (i, host) in hosts.iter().enumerate() {
            list = list.child(self.host_row(i, host, cx));
            for (j, space) in host.spaces.iter().enumerate() {
                list = list.child(self.space_row(i * 4096 + j, host, space, cx));
            }
        }
        if !agents.is_empty() {
            list = list.child(heading(AGENTS_HEADING));
            for (k, line) in agents.iter().enumerate() {
                let host = hosts
                    .iter()
                    .find(|h| h.host == line.row.identity.host && h.kind == line.row.identity.kind);
                list = list.child(self.agent_row(k, line, host, cx));
            }
        }
        let menu = self.render_long_press_menu(cx);
        Some(list.children(menu).into_any_element())
    }

    fn host_row(&self, i: usize, host: &HostEntry, cx: &mut Context<Self>) -> AnyElement {
        let (dot, color) = host_dot(host.link);
        let tap = SidebarTap::Host(host.host.clone());
        let press = PressedRow::Host(host.host.clone());
        div()
            .id(("sidebar-host", i))
            .relative()
            .h(px(row_height(HOST_ROW_HEIGHT, TEXT_SCALE)))
            .flex()
            .flex_col()
            .justify_center()
            .px_3()
            .when(host.selected, |d| d.bg(rgb(0x3b4252)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .min_w_0()
                    .child(div().text_color(rgb(color)).child(dot))
                    .child(div().min_w_0().truncate().child(host.host.clone())),
            )
            .child(second_line(host.detail().to_owned()))
            .child(long_press_layer(press, cx))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.tap_row(tap.clone(), window, cx);
            }))
            .into_any_element()
    }

    fn space_row(
        &self,
        id: usize,
        host: &HostEntry,
        space: &SpaceEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tap = SidebarTap::Space {
            host: host.host.clone(),
            id: space.id.clone(),
            boot: host.boot.clone(),
            generation: host.generation,
        };
        let tags = space.tags().into_iter().map(|tag| {
            div()
                .flex_shrink_0()
                .px_1()
                .text_sm()
                .bg(rgb(if tag == "Error" { 0xbf616a } else { 0xebcb8b }))
                .text_color(rgb(0x2e3440))
                .child(tag)
        });
        let press = PressedRow::Space {
            host: host.host.clone(),
            id: space.id.clone(),
            name: space.name.clone(),
        };
        div()
            .id(("sidebar-space", id))
            .relative()
            .h(px(row_height(SPACE_ROW_HEIGHT, TEXT_SCALE)))
            .flex()
            .flex_col()
            .justify_center()
            .pl_6()
            .pr_3()
            .when(space.current, |d| d.bg(rgb(0x3b4252)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .min_w_0()
                    .child(div().text_color(rgb(space.dot.color())).child("●"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(space.name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(format!("{} tabs", space.tabs)),
                    )
                    .children(tags),
            )
            .child(second_line(space.detail.clone()))
            .child(long_press_layer(press, cx))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.tap_row(tap.clone(), window, cx);
            }))
            .into_any_element()
    }

    fn agent_row(
        &self,
        k: usize,
        line: &AgentLine,
        host: Option<&HostEntry>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tap = SidebarTap::Agent {
            host: line.row.identity.host.clone(),
            pane: line.row.identity.pane.clone(),
            boot: host.map(|h| h.boot.clone()).unwrap_or_default(),
            generation: host.map_or(0, |h| h.generation),
        };
        let status = line.row.status;
        div()
            .id(("sidebar-agent", k))
            .h(px(row_height(AGENT_ROW_HEIGHT, TEXT_SCALE)))
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
                    .child(div().text_color(rgb(status.color())).child("●"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(line.shown.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(rgb(status.color()))
                            .child(status.label()),
                    ),
            )
            .child(second_line(line.place.clone()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.tap_row(tap.clone(), window, cx);
            }))
            .into_any_element()
    }
}
