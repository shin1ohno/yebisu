//! The rail column on screen. The numbers (sizes, badge places) come from
//! `rail.rs` and `rail_badges.rs`; this file only draws them.
use super::{
    EDGE_PAD, FOOTER_BUTTON_HEIGHT, ITEM_GAP, RAIL_FOOTER_HEIGHT, RailHost, RailItem, RailItemKind,
    RailMetrics, RailSpace, RailTarget, new_space_unavailable, rail_items, rail_metrics,
};
use crate::{
    app::Smoke,
    board::{Target, snapshot},
    layout::RAIL_WIDTH,
    live::Live,
    shell::{
        HerdrKind,
        rail_badges::{
            BADGE_MIN_WIDTH, BUTTON_WIDTH, BadgeTexts, DOT_SIZE, RailBadges, badge_spans,
            badge_texts, rail_badges,
        },
        sidebar_status::from_agent_status,
    },
};
use gpui::{AnyElement, Context, Div, FontWeight, Role, Stateful, div, font, prelude::*, px, rgb};
use sheprd_commands::Command;

const ACCENT: u32 = 0x88c0d0;
const NEEDS_BADGE: u32 = 0xebcb8b;
const ERROR_BADGE: u32 = 0xbf616a;
const BADGE_TEXT: u32 = 0x101419;
const RAIL_BG: u32 = 0x1c1c22;
const RULE: u32 = 0x2b2933;
const MUTED: u32 = 0xacb2bb;

/// Width of a badge label from the text system, bold at `pt`. The sum of the
/// glyph advances is exact for digits and `+`.
fn badge_measure(cx: &Context<Smoke>, pt: f32) -> impl Fn(&str) -> f32 + use<> {
    let text_system = cx.text_system().clone();
    let mut face = font(".SystemUIFont");
    face.weight = FontWeight::BOLD;
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
    /// Every host with its spaces, in the order the board uses.
    pub(super) fn rail_hosts(&self) -> Vec<RailHost> {
        let mut hosts: Vec<(&str, &Live, u64, bool)> = self
            .board
            .hidden
            .iter()
            .map(|(host, t)| (host.as_str(), &t.live, t.link.generation, t.core_mode))
            .collect();
        hosts.push((
            self.terminal.reconnect.target.as_str(),
            &self.terminal.live,
            self.terminal.link.generation,
            self.terminal.core_mode,
        ));
        hosts.sort_by(|a, b| a.0.cmp(b.0));
        hosts
            .into_iter()
            .filter(|(name, ..)| !name.is_empty())
            .map(|(name, live, generation, core)| {
                let shell = snapshot(live);
                RailHost {
                    name: name.to_owned(),
                    kind: if core {
                        HerdrKind::Herdr2
                    } else {
                        HerdrKind::Herdr1
                    },
                    connected: live.connected,
                    current: name == self.terminal.reconnect.target,
                    boot: shell.map(|s| s.boot_id.clone()).unwrap_or_default(),
                    generation,
                    spaces: shell
                        .map(|s| {
                            s.workspaces
                                .iter()
                                .map(|space| RailSpace {
                                    workspace_id: space.workspace_id.clone(),
                                    number: space.number,
                                    label: space.label.clone(),
                                    status: from_agent_status(space.agent_status),
                                    agents: s
                                        .agents
                                        .iter()
                                        .filter(|a| a.workspace_id == space.workspace_id)
                                        .map(|a| from_agent_status(a.agent_status))
                                        .collect(),
                                    focused: s.focused_workspace_id.as_ref()
                                        == Some(&space.workspace_id),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    /// The rail: a fixed `header` (`rail_header`), a scrolling list of hosts
    /// and spaces, then New space, Hosts, Settings and the expand button,
    /// fixed.
    pub(super) fn rail_column(&mut self, header: f32, cx: &mut Context<Self>) -> AnyElement {
        let metrics = rail_metrics(1.0);
        let items = rail_items(&self.rail_hosts());
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
            .children(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| rail_item(index, item, &metrics, &measure, cx)),
            );
        let kind = if self.terminal.core_mode {
            HerdrKind::Herdr2
        } else {
            HerdrKind::Herdr1
        };
        let expand_reason =
            super::expand_unavailable(self.sidebar_user_mode(), self.shell.frame.sidebar_collapsed);
        let new_space = new_space_unavailable(kind, self.terminal.live.connected).is_none();
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
            .bg(rgb(RAIL_BG))
            .child(div().h(px(header)).flex_none())
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
                        footer_button("rail-new-space", "New space", new_space).when(
                            new_space,
                            |button| {
                                button.on_click(cx.listener(|this, _, _, cx| {
                                    if let Err(e) = this.run_action(Command::Workspace) {
                                        this.terminal.live.status = e.to_string();
                                    }
                                    cx.notify();
                                }))
                            },
                        ),
                    )
                    .child(
                        footer_button("rail-hosts", "Hosts", true).on_click(cx.listener(
                            |this, _, window, cx| {
                                // The footer's path (L2b): open L7's sheet and
                                // hand the keys to it, so a sheet over a live
                                // terminal takes the typing (review 9714 F1).
                                this.footer_tap(
                                    crate::shell::footer::FooterItem::Hosts,
                                    window,
                                    cx,
                                );
                            },
                        )),
                    )
                    .child(
                        footer_button("rail-settings", crate::shell::footer::SETTINGS, true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_settings_modal(window, cx)
                            }))
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::RAIL_SETTINGS,
                                crate::shell::footer::SETTINGS,
                            )),
                    )
                    .child(
                        footer_button("rail-expand", "»", expand_reason.is_none())
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

fn footer_button(id: &'static str, label: &'static str, enabled: bool) -> Stateful<Div> {
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
        .text_size(px(super::LABEL_PT))
        .text_color(rgb(if enabled { MUTED } else { 0x5b6270 }))
        .child(label)
}

fn badge(text: &str, span: (f32, f32), color: u32, metrics: &RailMetrics) -> AnyElement {
    div()
        .absolute()
        .top(px(4.))
        .left(px(span.0))
        .w(px(span.1 - span.0))
        .h(px(metrics.badge_height))
        .rounded(px(metrics.badge_height / 2.))
        .bg(rgb(color))
        .text_color(rgb(BADGE_TEXT))
        .text_size(px(metrics.badge_pt))
        .font_weight(FontWeight::BOLD)
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
        Some(status) => div().bg(rgb(status.color())),
        None if item.connected => div().bg(rgb(0xa3be8c)),
        None => div().border_2().border_color(rgb(MUTED)),
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
        .gap(px(4.))
        .rounded(px(6.))
        .when(item.current, |button| {
            button.bg(rgb(RULE)).border_1().border_color(rgb(ACCENT))
        })
        .child(dot.size(px(DOT_SIZE)).rounded_full().flex_none())
        .child(
            div()
                .w(px(BUTTON_WIDTH - 2. * 4.))
                .text_size(px(metrics.label_pt))
                .text_center()
                .line_clamp(2)
                .text_color(rgb(if item.kind == RailItemKind::Host && !item.connected {
                    MUTED
                } else {
                    0xd8dee9
                }))
                .child(item.name.clone()),
        )
        .children(
            texts
                .left
                .as_deref()
                .zip(spans.left)
                .map(|(text, span)| badge(text, span, ERROR_BADGE, metrics)),
        )
        .children(
            texts
                .right
                .as_deref()
                .zip(spans.right)
                .map(|(text, span)| badge(text, span, NEEDS_BADGE, metrics)),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            window.dismiss_virtual_keyboard();
            match &target {
                RailTarget::Host(host) => this.select_host(host),
                RailTarget::Space {
                    host,
                    workspace_id,
                    boot,
                    generation,
                } => {
                    if let Err(e) = this.navigate_board(
                        host,
                        Target::Space(workspace_id.clone()),
                        boot,
                        *generation,
                    ) {
                        this.terminal.live.status = e.to_string();
                    }
                }
            }
            cx.notify();
        }))
        .into_any_element()
}
