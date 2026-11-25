//! L3: draws the sidebar list and routes its taps. Owner: L3.
use super::{
    AGENT_ROW_HEIGHT, HOST_ROW_HEIGHT, SPACE_ROW_HEIGHT, SidebarState, accessibility_name,
    agent_icon,
    icons::{self, Icon},
    long_press::{AgentMenu, PressedRow, agent_press_layer, long_press_layer},
    model::{
        AgentLine, HostEntry, HostLink, OfflineReason, agents_list_with_pins, host_link,
        order_hosts, record_live, snapshot_content,
    },
};
use crate::{
    app::Smoke,
    board::{Target, snapshot},
    layout::{SIDEBAR_HEADER_HEIGHT, SIDEBAR_HEADER_PADDING},
    shell::{
        HerdrKind, filter,
        herdr2_frame::sidebar::{CoreRow, herdr2_hosts},
        rename::{RenameAnchor, RenameObject},
        sidebar_list::{ListHost, ListRow, ListSpace, merge, sources},
        style::{color, metric, radius, text, tint, weight},
    },
    terminal_view::TerminalView,
};
use gpui::{AnyElement, Context, Hsla, Role, div, prelude::*, px, rgb};

/// The list's heading and Simulator scroll anchor.
pub(crate) const HOSTS_HEADING: &str = "HOSTS";
pub(crate) const AGENTS_HEADING: &str = "AGENTS";

// Main.dc.html:25, :52, :66, :101. ModsMax.dc.html:85.
const HEADER_GAP: f32 = 4.;
const SPACE_TRAILING: f32 = metric::INSET;
const SPACE_LINE_GAP: f32 = metric::SEGMENT_INSET;
const NEEDS_BADGE_PADDING: f32 = metric::TIGHT_GAP;
const ERROR_BADGE_PADDING: f32 = 5.;
const PINNED_TRAILING: f32 = 36.;

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

/// The sidebar's rows for a query (`Smoke::sidebar_rows`).
pub(in crate::shell) struct SidebarRows {
    pub(in crate::shell) hosts: Vec<HostEntry>,
    pub(in crate::shell) rows: Vec<ListRow>,
    pub(in crate::shell) agents: Vec<AgentLine>,
}

/// Host rows for every reader this app holds, in the saved Hosts order.
/// The receive order is recorded when updates arrive (`record_live`); this
/// records again only to cover a reader no update has reached yet.
pub(crate) fn host_entries<'a>(
    state: &SidebarState,
    current: &TerminalView,
    hidden: impl IntoIterator<Item = (&'a String, &'a TerminalView)>,
) -> Vec<HostEntry> {
    let mut hosts = Vec::new();
    for (host, terminal) in hidden
        .into_iter()
        .map(|(h, t)| (h.as_str(), t))
        .chain(std::iter::once((
            current.reconnect.target.as_str(),
            current,
        )))
    {
        if host.is_empty() {
            continue;
        }
        let kind = terminal.herdr_kind();
        let live = &terminal.live;
        let link = host_link(live);
        let mut entry = HostEntry::new(host, kind, link);
        entry.selected = std::ptr::eq(terminal, current);
        entry.generation = terminal.link.generation;
        record_live(state, host, kind, live);
        if let (HostLink::Connected, HerdrKind::Herdr1, Some(snap)) = (link, kind, snapshot(live)) {
            entry.boot = snap.boot_id.clone();
            let (mut spaces, agents) = snapshot_content(host, snap, state);
            for space in &mut spaces {
                space.current &= entry.selected;
            }
            entry = entry.with_content(spaces, agents);
        }
        hosts.push(entry);
    }
    order_hosts(&mut hosts, &state.host_order);
    hosts
}

/// Keep the identity of the connection the rendered space came from.
#[cfg(test)]
pub(super) fn space_tap(host: &HostEntry, space: &super::model::SpaceEntry) -> SidebarTap {
    ListSpace {
        host: host.host.clone(),
        kind: host.kind,
        entry: space.clone(),
        boot: host.boot.clone(),
        generation: host.generation,
    }
    .tap()
}

fn heading(text: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(px(metric::HEADING_ROW))
        .px(px(metric::SIDE))
        .text_size(px(text::SMALL))
        .font_weight(weight::SEMIBOLD)
        .text_color(rgb(color::MUTED))
        .child(text)
        .when(text == AGENTS_HEADING, |row| {
            row.mt(px(metric::HEADING_GAP))
                .child(div().font_weight(weight::MEDIUM).child("Needs you first"))
        })
}

/// The QA accessibility mark of a host row (UI test redesign S6): only P15's
/// fixture hosts have one, labelled with the host name.
fn host_mark(host: &str) -> Option<AnyElement> {
    use crate::qa_ax::ids;
    match host {
        "qa-alpha" => crate::qa_ax::mark(ids::SIDEBAR_HOST_QA_ALPHA, "qa-alpha"),
        "qa-beta" => crate::qa_ax::mark(ids::SIDEBAR_HOST_QA_BETA, "qa-beta"),
        _ => None,
    }
}

fn second_line(text: String) -> gpui::Div {
    div()
        .text_size(px(text::SECONDARY))
        .text_color(rgb(color::MUTED))
        .min_w_0()
        .truncate()
        .child(text)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Dot {
    pub(super) color: u32,
    pub(super) stroke: f32,
}

impl Dot {
    fn filled(color: u32) -> Self {
        Self { color, stroke: 0. }
    }

    fn draw(self) -> gpui::Div {
        div()
            .size(px(metric::DOT))
            .flex_none()
            .rounded_full()
            .when(self.stroke == 0., |dot| dot.bg(rgb(self.color)))
            .when(self.stroke > 0., |mut dot| {
                let stroke = Some(px(self.stroke).into());
                let widths = &mut dot.style().border_widths;
                widths.top = stroke;
                widths.right = stroke;
                widths.bottom = stroke;
                widths.left = stroke;
                dot.border_color(rgb(self.color))
            })
    }
}

pub(super) fn host_dot(link: HostLink) -> Dot {
    match link {
        HostLink::Connected => Dot::filled(color::GREEN),
        HostLink::Connecting => Dot {
            color: color::YELLOW,
            stroke: metric::RING,
        },
        HostLink::Offline(OfflineReason::NotConnected) => Dot {
            color: color::MUTED,
            stroke: metric::RING,
        },
        HostLink::Offline(_) => Dot::filled(color::RED),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Badge {
    pub(super) ink: u32,
    pub(super) fill: Option<u32>,
    pub(super) outline: bool,
}

impl Badge {
    pub(super) fn for_tag(tag: &str) -> Self {
        if tag == "Error" {
            Self {
                ink: color::RED,
                fill: None,
                outline: true,
            }
        } else {
            Self {
                ink: color::BACKGROUND,
                fill: Some(color::YELLOW),
                outline: false,
            }
        }
    }

    fn draw(self, name: &'static str) -> gpui::Div {
        div()
            .flex_none()
            .rounded(px(radius::BADGE))
            .text_size(px(text::SMALL))
            .font_weight(weight::BOLD)
            .text_color(rgb(self.ink))
            .when_some(self.fill, |tag, color| tag.bg(rgb(color)))
            .when(self.outline, |tag| {
                tag.px(px(ERROR_BADGE_PADDING))
                    .border_1()
                    .border_color(rgb(self.ink))
            })
            .when(!self.outline, |tag| {
                tag.px(px(NEEDS_BADGE_PADDING)).py(px(metric::LINE))
            })
            .child(name)
    }
}

impl Smoke {
    /// Taps go through PR 40's `input_transition`, like the board's.
    pub(crate) fn sidebar_tap(&mut self, tap: SidebarTap) -> crate::Result<()> {
        self.input_transition(|this| match tap {
            SidebarTap::Host(host) => {
                this.shell.sidebar.reveal_host(&host);
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

    pub(super) fn tap_row(
        &mut self,
        tap: SidebarTap,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        window.dismiss_virtual_keyboard();
        if let Err(e) = self.sidebar_tap(tap) {
            self.terminal.live.status = e.to_string();
        }
        cx.notify();
    }

    pub(crate) fn render_sidebar_header(
        &mut self,
        leading: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.flush_sidebar_pins(cx);
        let leading = if leading == SIDEBAR_HEADER_PADDING {
            metric::INLINE_GAP
        } else {
            leading
        };
        div()
            .id("sidebar-header")
            .flex_none()
            .h(px(SIDEBAR_HEADER_HEIGHT))
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .pl(px(leading))
            .pr(px(metric::INLINE_GAP))
            .child(
                Icon::Collapse
                    .button("shell-sidebar-toggle", "Collapse sidebar", true)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_sidebar();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("sidebar-title")
                    .debug_selector(|| "sidebar-title".into())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::NAME))
                    .font_weight(weight::SEMIBOLD)
                    .child("yebisu"),
            )
            .child(self.render_filter_field(cx))
            .child(self.render_new_space_button(cx))
            .into_any_element()
    }

    /// What the list shows for `query`: the host entries, the host rows with
    /// the spaces the query and the collapsed hosts leave (in drawing order),
    /// and the AGENTS lines. The search panel builds its results from the
    /// same rows (`search::results::build`).
    pub(in crate::shell) fn sidebar_rows(&mut self, query: &str) -> SidebarRows {
        let state = &mut self.shell.sidebar;
        state.notice_selected_host(&self.terminal.reconnect.target);
        let mut hosts = sources(
            host_entries(
                state,
                &self.terminal,
                self.board.hidden.iter().chain(&self.board.hidden_core),
            ),
            herdr2_hosts(state, &self.terminal, &self.board.hidden_core),
        );
        let saved = &self.setup.hosts;
        for host in saved {
            if !hosts.iter().any(|entry| entry.host == *host) {
                hosts.push(HostEntry::new(
                    host,
                    HerdrKind::Herdr1,
                    HostLink::Offline(OfflineReason::NotConnected),
                ));
            }
        }
        if state.host_order.is_empty() {
            order_hosts(&mut hosts, saved);
        } else {
            order_hosts(&mut hosts, &state.host_order);
        }
        let agents = agents_list_with_pins(&hosts, &|id, boot| {
            self.shell.sidebar.pins.borrow().index(id, boot)
        });
        let agents = filter::filter_lines(agents, query);
        let rows = merge(&hosts, &self.setup)
            .into_iter()
            .filter(|row| match row {
                ListRow::Host(_) => true,
                ListRow::Space(space) => {
                    !self.shell.sidebar.host_collapsed(&space.host)
                        && filter::matches(query, &format!("{} {}", space.entry.name, space.host))
                }
            })
            .collect();
        SidebarRows {
            hosts,
            rows,
            agents,
        }
    }

    /// The same sidebar serves both daemon types, with the sessions supplied
    /// by Herdr 2 replacing their sessionless connection entries.
    pub(crate) fn render_sidebar_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.sidebar_filter().to_owned();
        let SidebarRows {
            hosts,
            rows,
            agents,
        } = self.sidebar_rows(&query);
        let mut list = div()
            .id("sidebar-list")
            .debug_selector(|| "sidebar-list".into())
            .flex()
            .flex_col()
            .w_full()
            .child(heading(HOSTS_HEADING))
            .children(crate::qa_ax::mark(
                crate::qa_ax::ids::SIDEBAR_LIST,
                HOSTS_HEADING,
            ));
        // While a filter is set (also with results), the list says so and
        // offers to clear it, in the normal and the compact layout.
        list = list.children(self.render_filter_banner(cx));
        for row in rows {
            match row {
                ListRow::Host(host) => list = list.child(self.host_row(&host, cx)),
                ListRow::Space(space) => list = list.child(self.space_row(&space, cx)),
            }
        }
        if !agents.is_empty() {
            list = list.child(heading(AGENTS_HEADING));
            for line in &agents {
                if let Some(host) = hosts
                    .iter()
                    .find(|h| h.host == line.row.identity.host && h.kind == line.row.identity.kind)
                {
                    list = list.child(self.agent_row(line, host, cx));
                }
            }
        }
        let menu = self.render_long_press_menu(cx);
        let agent_menu = self.render_agent_menu(cx);
        list.children(menu).children(agent_menu).into_any_element()
    }

    fn host_row(&self, host: &ListHost, cx: &mut Context<Self>) -> AnyElement {
        let target = host.name.destination().to_owned();
        let tap = SidebarTap::Host(target.clone());
        let press = PressedRow::Host(target.clone());
        let connection = host.link();
        let expanded =
            connection == HostLink::Connected && !self.shell.sidebar.host_collapsed(&target);
        let name = host.name.to_string();
        let arrow = if expanded { Icon::Down } else { Icon::Right };
        let label = format!("{} {name}", if expanded { "Collapse" } else { "Expand" });
        let retry_target = target.clone();
        let row_id = format!("sidebar-host-{target}");
        let chevron_id = format!("sidebar-chevron-{target}");
        let qa = host_mark(&target);
        div()
            .id(gpui::ElementId::Name(row_id.clone().into()))
            .debug_selector(move || row_id.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(self.shell.sidebar.sidebar_row_height(HOST_ROW_HEIGHT)))
            .pl(px(metric::SIDE))
            .pr(px(metric::INLINE_GAP))
            .gap(px(metric::ROW_GAP))
            .child(host_dot(connection).draw())
            .child(
                div()
                    .id("host-title")
                    .role(Role::Button)
                    .aria_label(name.clone())
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .h_full()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::NAME))
                            .font_weight(weight::BOLD)
                            .child(name),
                    )
                    .child(second_line(host.connection_detail()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.tap_row(tap.clone(), window, cx);
                        if matches!(connection, HostLink::Offline(_)) {
                            this.start_host_connection(retry_target.clone(), cx);
                        }
                    })),
            )
            .child(
                div()
                    .id(gpui::ElementId::Name(chevron_id.clone().into()))
                    .debug_selector(move || chevron_id.clone())
                    .role(Role::Button)
                    .aria_label(label)
                    .aria_expanded(expanded)
                    .size(px(metric::TOUCH))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .child(arrow.draw(color::MUTED))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        match connection {
                            HostLink::Connected => this.shell.sidebar.toggle_host(&target),
                            HostLink::Offline(_) => {
                                this.tap_row(SidebarTap::Host(target.clone()), window, cx);
                                this.start_host_connection(target.clone(), cx);
                            }
                            HostLink::Connecting => {}
                        }
                        cx.notify();
                    })),
            )
            .child(long_press_layer(press, cx))
            .children(qa)
            .into_any_element()
    }

    fn space_row(&self, row: &ListSpace, cx: &mut Context<Self>) -> AnyElement {
        let tap = row.tap();
        let (host, kind, boot, generation) =
            (row.host.clone(), row.kind, row.boot.clone(), row.generation);
        let space = &row.entry;
        let core_target = CoreRow::Space(space.id.clone());
        let tags = space
            .tags()
            .into_iter()
            .map(|tag| Badge::for_tag(tag).draw(tag));
        let press = PressedRow::Space {
            host: host.clone(),
            id: space.id.clone(),
            name: space.name.clone(),
        };
        let prefix = match kind {
            HerdrKind::Herdr1 => "sidebar-space",
            HerdrKind::Herdr2 => "sidebar-core-space",
        };
        let id = format!("{prefix}-{host}-{}", space.id);
        let icon_row = id.clone();
        let detail_id = format!("{id}-detail");
        div()
            .id(gpui::ElementId::Name(id.clone().into()))
            .debug_selector(move || id.clone())
            .role(Role::Button)
            .aria_label(space.name.clone())
            .aria_selected(space.current)
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .justify_center()
            .h(px(self.shell.sidebar.sidebar_row_height(SPACE_ROW_HEIGHT)))
            .pl(px(metric::SPACE_INDENT))
            .pr(px(SPACE_TRAILING))
            .gap(px(SPACE_LINE_GAP))
            .when(space.current, |row| {
                row.bg(Hsla::from(tint::CURRENT_ROW)).child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .h_full()
                        .w(px(metric::CURRENT_LINE))
                        .bg(rgb(color::ACCENT)),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(metric::INLINE_GAP))
                    .min_w_0()
                    .child(Dot::filled(space.dot.dot_color()).draw())
                    .child(self.space_name(row, cx))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(text::SMALL))
                            .text_color(rgb(color::MUTED))
                            .child(space.tab_count()),
                    )
                    .children(tags),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(agent_icon::SPACE.gap))
                    .min_w_0()
                    .pl(px(metric::DETAIL_INDENT))
                    .children(
                        space
                            .icon
                            .map(|icon| icon.draw(&icon_row, agent_icon::SPACE)),
                    )
                    .child(
                        second_line(space.detail.clone())
                            .debug_selector(move || detail_id)
                            .flex_1(),
                    ),
            )
            .when(kind == HerdrKind::Herdr1, |row| {
                row.child(long_press_layer(press, cx))
            })
            .when(kind == HerdrKind::Herdr1 && space.current, |row| {
                row.children(crate::qa_ax::mark(
                    crate::qa_ax::ids::SIDEBAR_CURRENT_SPACE,
                    "Current space",
                ))
            })
            .on_click(cx.listener(move |this, _, window, cx| match kind {
                HerdrKind::Herdr1 => this.tap_row(tap.clone(), window, cx),
                HerdrKind::Herdr2 => {
                    window.dismiss_virtual_keyboard();
                    this.tap_core_row(&host, &boot, generation, core_target.clone(), cx);
                }
            }))
            .into_any_element()
    }

    fn agent_row(&self, line: &AgentLine, host: &HostEntry, cx: &mut Context<Self>) -> AnyElement {
        let id = line.row.identity.clone();
        let tap = SidebarTap::Agent {
            host: id.host.clone(),
            pane: id.pane.clone(),
            boot: host.boot.clone(),
            generation: host.generation,
        };
        let pinned = self.shell.sidebar.is_agent_pinned(&id, &host.boot);
        let status = line.row.status;
        let host_name = self.setup.host_name(&id.host);
        let spoken = accessibility_name(&line.row, &host_name);
        let press = AgentMenu {
            id: id.clone(),
            tap: tap.clone(),
            at: Default::default(),
        };
        let prefix = match id.kind {
            HerdrKind::Herdr1 => "sidebar-agent",
            HerdrKind::Herdr2 => "sidebar-core-agent",
        };
        let row_id = format!("{prefix}-{}-{}", id.host, id.pane.escape_default());
        let icon_row = row_id.clone();
        let name_id = format!("{row_id}-name");
        let boot = host.boot.clone();
        let generation = host.generation;
        div()
            .id(gpui::ElementId::Name(row_id.clone().into()))
            .debug_selector(move || row_id.clone())
            .role(Role::Button)
            .aria_label(if pinned {
                format!("Pinned: {spoken}")
            } else {
                spoken
            })
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(metric::ROW_GAP))
            .px(px(metric::SIDE))
            .when(pinned, |row| {
                row.pr(px(PINNED_TRAILING)).child(
                    div()
                        .absolute()
                        .right(px(metric::SIDE))
                        .top(px((self
                            .shell
                            .sidebar
                            .sidebar_row_height(AGENT_ROW_HEIGHT)
                            - icons::PIN)
                            / 2.))
                        .child(Icon::Pin.draw(color::TEXT)),
                )
            })
            .h(px(self.shell.sidebar.sidebar_row_height(AGENT_ROW_HEIGHT)))
            .child(Dot::filled(status.dot_color()).draw())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .line_height(px(text::AGENT_LINE))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(metric::INLINE_GAP))
                            .min_w_0()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap(px(agent_icon::AGENTS.gap))
                                    .child(line.row.icon.draw(&icon_row, agent_icon::AGENTS))
                                    .child(
                                        self.renamable(
                                            div()
                                                .debug_selector(move || name_id)
                                                .flex_1()
                                                .min_w_0()
                                                .truncate(),
                                            &host.boot,
                                            RenameAnchor::AgentRow,
                                            RenameObject::Pane(id.pane.clone()),
                                            line.shown.clone(),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(text::SECONDARY))
                                    .font_weight(status.label_weight())
                                    .text_color(rgb(status.label_color()))
                                    .child(status.label()),
                            ),
                    )
                    .child(second_line(line.place(&host_name))),
            )
            .when(id.kind == HerdrKind::Herdr1, |row| {
                row.child(agent_press_layer(press, cx))
            })
            .on_click(cx.listener(move |this, _, window, cx| match id.kind {
                HerdrKind::Herdr1 => this.tap_row(tap.clone(), window, cx),
                HerdrKind::Herdr2 => {
                    window.dismiss_virtual_keyboard();
                    this.tap_core_row(
                        &id.host,
                        &boot,
                        generation,
                        CoreRow::Agent(id.pane.clone()),
                        cx,
                    );
                }
            }))
            .into_any_element()
    }

    fn tap_core_row(
        &mut self,
        host: &str,
        boot: &str,
        generation: u64,
        target: CoreRow,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.open_herdr2_row(host, boot, generation, target, cx) {
            self.terminal.live.status = error.to_string();
        }
        cx.notify();
    }
}
