//! The rail column on screen. The numbers (sizes, badge places) come from
//! `rail.rs` and `rail_badges.rs`; this file only draws them.
use super::{
    DIVIDER_WIDTH, EDGE_PAD, FOOTER_BUTTON_HEIGHT, HOST_DIVIDER_MARGIN, ITEM_GAP,
    RAIL_FOOTER_HEIGHT, RailHost, RailItem, RailItemKind, RailMetrics, RailSpace,
    agents::core_agent_session,
    icons::{RailIcon, icon},
    menu::long_press_layer,
    rail_items, rail_metrics,
};
use crate::{
    app::Smoke,
    board::snapshot,
    layout::RAIL_WIDTH,
    shell::{
        HerdrKind,
        rail_badges::{
            BADGE_MIN_WIDTH, BUTTON_WIDTH, BadgeTexts, DOT_SIZE, RailBadges, badge_spans,
            badge_texts, rail_badges,
        },
        sidebar::{agent_icon, icons::Icon as SidebarIcon},
        sidebar_status::from_agent_status,
        style::{color, metric, radius, weight},
    },
    terminal_view::TerminalView,
};
use gpui::{AnyElement, Context, Div, Role, Stateful, div, font, prelude::*, px, rgb};

// Rail.dc.html:30, :41.
const ROW_CONTENT_GAP: f32 = 4.0;
const BADGE_TOP: f32 = 4.0;

/// Width of a badge label from the text system, bold at `pt`. The sum of the
/// glyph advances is exact for digits and `+`.
fn badge_measure(cx: &Context<Smoke>, pt: f32) -> impl Fn(&str) -> f32 + use<> {
    let text_system = cx.text_system().clone();
    let mut face = font(".SystemUIFont");
    face.weight = weight::BOLD;
    let face_id = text_system.resolve_font(&face);
    move |label: &str| {
        label
            .chars()
            .map(|ch| {
                text_system
                    .advance(face_id, px(pt), ch)
                    .map(|size| f32::from(size.width))
                    .unwrap_or(pt * 0.62)
            })
            .sum()
    }
}

impl Smoke {
    /// Gather each host's connections, with the preferred one first.
    pub(crate) fn rail_hosts(&self) -> Vec<RailHost> {
        let saved = &self.setup.hosts;
        let mut hosts = self
            .board
            .terminals(&self.terminal)
            .map(|terminal| terminal.reconnect.target.as_str())
            .collect::<Vec<_>>();
        hosts.extend(saved.iter().map(String::as_str));
        hosts.retain(|name| !name.is_empty());
        hosts.sort_by_key(|&name| {
            (
                saved
                    .iter()
                    .position(|saved| saved == name)
                    .unwrap_or(usize::MAX),
                name,
            )
        });
        hosts.dedup();
        let core_hosts = self.rail_core_hosts();
        let row = |name: &str, terminal: Option<&TerminalView>| {
            let kind = terminal.map_or(HerdrKind::Herdr1, TerminalView::herdr_kind);
            let shown = terminal.and_then(|terminal| snapshot(&terminal.live));
            let spaces = match kind {
                HerdrKind::Herdr1 => shown
                    .map(|shown| {
                        shown
                            .workspaces
                            .iter()
                            .map(|space| RailSpace {
                                workspace_id: space.workspace_id.clone(),
                                number: space.number,
                                label: space.label.clone(),
                                agents: shown
                                    .agents
                                    .iter()
                                    .filter(|agent| agent.workspace_id == space.workspace_id)
                                    .map(|agent| from_agent_status(agent.agent_status))
                                    .collect(),
                                focused: shown.focused_workspace_id.as_ref()
                                    == Some(&space.workspace_id),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                HerdrKind::Herdr2 => core_hosts
                    .iter()
                    .find(|host| host.host == name)
                    .map(|host| {
                        host.spaces
                            .iter()
                            .enumerate()
                            .map(|(index, space)| RailSpace {
                                workspace_id: space.id.clone(),
                                number: index + 1,
                                label: space.name.clone(),
                                agents: host
                                    .agents
                                    .iter()
                                    .filter(|line| {
                                        core_agent_session(&line.row.identity.pane) == space.id
                                    })
                                    .map(|line| line.row.status)
                                    .collect(),
                                focused: space.current,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            RailHost {
                name: name.to_owned(),
                kind,
                connected: terminal.is_some_and(|terminal| terminal.live.connected),
                current: terminal.is_some_and(|terminal| std::ptr::eq(terminal, &self.terminal)),
                boot: shown.map(|shown| shown.boot_id.clone()).unwrap_or_default(),
                generation: terminal.map_or(0, |terminal| terminal.link.generation),
                spaces,
            }
        };
        let mut rows = Vec::new();
        for name in hosts {
            let preferred = row(name, self.rail_terminal(name));
            let other = match preferred.kind {
                HerdrKind::Herdr1 => HerdrKind::Herdr2,
                HerdrKind::Herdr2 => HerdrKind::Herdr1,
            };
            rows.push(preferred);
            if let Some(terminal) = self.terminal_for(name, other).filter(|t| t.live.connected) {
                rows.push(row(name, Some(terminal)));
            }
        }
        rows
    }

    /// Fixed New space header, scrolling hosts and spaces, and fixed footer.
    pub(super) fn rail_column(&mut self, header: f32, cx: &mut Context<Self>) -> AnyElement {
        self.load_sidebar_agent_pins(cx);
        let metrics = rail_metrics(1.0);
        let mut items = rail_items(&self.rail_hosts(), &self.setup);
        let mut agents = self.rail_agent_items();
        self.rail_names(&mut items);
        self.rail_names(&mut agents);
        let measure = badge_measure(cx, metrics.badge_pt);
        self.record_rail_badges(&items, &measure);
        let list = div()
            .id("rail-list")
            .flex()
            .flex_col()
            .items_center()
            .gap(px(ITEM_GAP))
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .track_scroll(self.shell.indicators.rail.handle())
            .children(items.iter().enumerate().flat_map(|(index, item)| {
                let divider =
                    (index > 0 && item.kind == RailItemKind::Host).then(|| host_divider(index));
                divider.into_iter().chain(std::iter::once(rail_item(
                    index + 1,
                    item,
                    &metrics,
                    &measure,
                    cx,
                )))
            }))
            .when(!agents.is_empty(), |list| {
                list.child(divider())
                    .children(agents.iter().enumerate().map(|(index, item)| {
                        rail_item(items.len() + index + 1, item, &metrics, &measure, cx)
                    }))
            })
            .child(self.shell.indicators.rail.thumb());
        let expand_reason =
            super::expand_unavailable(self.sidebar_user_mode(), self.shell.frame.sidebar_collapsed);
        let header_control = rail_button("rail-new-space", "New space", RailIcon::NewSpace, true)
            // The window-controls regression observes the first interactive rail item.
            .debug_selector(|| "rail-item-0".to_owned())
            .on_click(cx.listener(|this, _, _, cx| this.open_new_space(cx)));
        div()
            .id("rail")
            .role(Role::Group)
            .aria_label("Hosts and spaces rail")
            .w(px(RAIL_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            // The frame draws the 1 pt rule between the sidebar and the column.
            .bg(rgb(color::SURFACE))
            .child(
                div()
                    .pt(px(header))
                    .pb(px(ITEM_GAP))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(ITEM_GAP))
                    .child(header_control)
                    .child(divider().debug_selector(|| "rail-header-rule".to_owned())),
            )
            .child(list)
            .child(
                div()
                    .h(px(RAIL_FOOTER_HEIGHT))
                    .flex_none()
                    .pt(px(ITEM_GAP))
                    .pb(px(EDGE_PAD))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(ITEM_GAP))
                    .child(
                        rail_button("rail-hosts", "Hosts", RailIcon::Hosts, true).on_click(
                            cx.listener(|this, _, window, cx| {
                                // The footer's path (L2b): open L7's sheet and
                                // hand the keys to it, so a sheet over a live
                                // terminal takes the typing (review 9714 F1).
                                this.footer_tap(
                                    crate::shell::footer::FooterItem::Hosts,
                                    window,
                                    cx,
                                );
                            }),
                        ),
                    )
                    .child(
                        rail_button(
                            "rail-settings",
                            crate::shell::footer::SETTINGS,
                            RailIcon::Settings,
                            true,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.begin_settings_modal(window, cx)
                            }),
                        )
                        .children(crate::qa_ax::mark(
                            crate::qa_ax::ids::RAIL_SETTINGS,
                            crate::shell::footer::SETTINGS,
                        )),
                    )
                    .child(
                        rail_button(
                            "rail-expand",
                            "Expand sidebar",
                            RailIcon::Expand,
                            expand_reason.is_none(),
                        )
                        .aria_label(match expand_reason {
                            Some(reason) => format!("Expand sidebar, {reason}"),
                            None => "Expand sidebar".to_owned(),
                        })
                        .when(expand_reason.is_none(), |button| {
                            button.on_click(cx.listener(|this, _, _, cx| {
                                this.expand_sidebar();
                                cx.notify();
                            }))
                        }),
                    ),
            )
            .children(self.render_rail_menu(cx))
            .into_any_element()
    }

    /// Writes `{"kind":"rail_badges","mode":"combined"}` once when a button
    /// first needs the combined badge, and the measured widths once (plan v2
    /// 7.9: the PR reports them).
    fn record_rail_badges(&mut self, items: &[RailItem], measure: &dyn Fn(&str) -> f32) {
        if !self.shell.rail.widths_recorded {
            self.shell.rail.widths_recorded = true;
            self.observation.record(serde_json::json!({
                "kind": "rail_badge_widths",
                "nine": measure("9"),
                "ninety_nine": measure("99"),
                "ninety_nine_plus": measure("99+"),
                "min_width": BADGE_MIN_WIDTH,
            }));
        }
        let combined = items
            .iter()
            .any(|i| rail_badges(i.needs, i.error, measure) == RailBadges::Combined);
        let mode = if combined {
            RailBadges::Combined
        } else {
            RailBadges::Two
        };
        if self.shell.rail.recorded_badges != Some(mode) && combined {
            self.observation
                .record(serde_json::json!({"kind":"rail_badges","mode":"combined"}));
        }
        self.shell.rail.recorded_badges = Some(mode);
    }
}

fn rail_button(
    id: &'static str,
    label: &'static str,
    mark: RailIcon,
    enabled: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .role(Role::Button)
        .aria_label(label)
        .when(!enabled, |button| {
            button.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
        .w(px(BUTTON_WIDTH))
        .h(px(FOOTER_BUTTON_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(icon(mark, enabled))
}

fn divider() -> Div {
    div()
        .flex_none()
        .w(px(DIVIDER_WIDTH))
        .h(px(metric::LINE))
        .bg(rgb(color::RAISED))
}

fn host_divider(index: usize) -> AnyElement {
    divider()
        .debug_selector(move || format!("rail-host-rule-{index}"))
        .my(px(HOST_DIVIDER_MARGIN))
        .into_any_element()
}

fn badge(text: &str, span: (f32, f32), color: u32, metrics: &RailMetrics) -> AnyElement {
    div()
        .absolute()
        .top(px(BADGE_TOP))
        .left(px(span.0))
        .w(px(span.1 - span.0))
        .h(px(metrics.badge_height))
        .rounded(px(metrics.badge_height / 2.))
        .bg(rgb(color))
        .text_color(rgb(color::BACKGROUND))
        .text_size(px(metrics.badge_pt))
        .font_weight(weight::BOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(text.to_owned())
        .into_any_element()
}

fn rail_item(
    index: usize,
    item: &RailItem,
    metrics: &RailMetrics,
    measure: &dyn Fn(&str) -> f32,
    cx: &mut Context<Smoke>,
) -> AnyElement {
    let mode = rail_badges(item.needs, item.error, measure);
    let texts: BadgeTexts = badge_texts(item.needs, item.error, mode);
    let spans = badge_spans(&texts, measure);
    let dot = match item.status {
        Some(status) => div().bg(rgb(status.dot_color())),
        None if item.connected => div().bg(rgb(color::GREEN)),
        None => div().border_2().border_color(rgb(color::MUTED)),
    };
    let target = item.target.clone();
    div()
        .id(("rail-item", index))
        .debug_selector(move || format!("rail-item-{index}"))
        .role(Role::Button)
        .aria_label(item.accessibility_label())
        .relative()
        .w(px(BUTTON_WIDTH))
        .h(px(metrics.item_height))
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(ROW_CONTENT_GAP))
        .rounded(px(radius::RAIL_ITEM))
        .when(item.current, |button| {
            button
                .bg(rgb(color::RAISED))
                .border_1()
                .border_color(rgb(color::ACCENT))
        })
        .child(dot.size(px(DOT_SIZE)).rounded_full().flex_none())
        .when(item.pinned, |button| {
            button.child(
                div()
                    .absolute()
                    .top(px(BADGE_TOP))
                    .right(px(EDGE_PAD))
                    .child(SidebarIcon::Pin.draw(color::MUTED)),
            )
        })
        .child(
            div()
                .w(px(BUTTON_WIDTH - 2. * ROW_CONTENT_GAP))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(agent_icon::RAIL.gap))
                .children(
                    item.icon
                        .map(|icon| icon.draw(&format!("rail-item-{index}"), agent_icon::RAIL)),
                )
                .child(
                    div()
                        .debug_selector(move || format!("rail-label-{index}"))
                        .when(item.icon.is_none(), |label| label.w_full())
                        .min_w_0()
                        .text_size(px(metrics.label_pt))
                        .text_center()
                        .truncate()
                        .text_color(rgb(if item.kind == RailItemKind::Host && !item.connected {
                            color::MUTED
                        } else {
                            color::TEXT
                        }))
                        .child(item.name.clone()),
                ),
        )
        .children(
            texts
                .left
                .as_deref()
                .zip(spans.left)
                .map(|(text, span)| badge(text, span, color::RED, metrics)),
        )
        .children(
            texts
                .right
                .as_deref()
                .zip(spans.right)
                .map(|(text, span)| badge(text, span, color::YELLOW, metrics)),
        )
        .child(long_press_layer(item.clone(), cx))
        .when(
            item.current && item.kind == RailItemKind::Space && item.herdr == HerdrKind::Herdr1,
            |button| {
                button.children(crate::qa_ax::mark(
                    crate::qa_ax::ids::RAIL_CURRENT_SPACE,
                    "Current space",
                ))
            },
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.open_rail_target(&target, window, cx);
            cx.notify();
        }))
        .into_any_element()
}
