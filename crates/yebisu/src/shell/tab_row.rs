//! L2a: the 36 pt tab row. Owner: L2a.
//!
//! Tabs scroll in their strip; New tab follows the last visible tab, and the
//! split and overflow buttons stay at the right edge. Each tab carries its own 32×36 × button, a separate hit
//! from the tab body (S-9).
pub(super) mod icons;
pub(crate) mod menu;

use super::{
    rename::RenameObject,
    sidebar_status::from_agent_status,
    style::{color, metric, radius, shadow, tint, weight},
};
use crate::{
    app::Smoke,
    board::{Target, snapshot},
    layout::TAB_ROW_HEIGHT,
};
use gpui::{AnyElement, Context, MouseButton, Role, deferred, div, prelude::*, px, rgb};
use icons::Icon;
use sheprd_client::{
    Method,
    protocol::{ClientShellSnapshot, ClientShellTab},
};
use sheprd_commands::Command;

/// Width of a tab's × button.
pub(crate) const CLOSE_WIDTH: f32 = 32.0;
/// Width of each pinned action (+, split, ...), canvas Main.dc.html.
pub(crate) const PINNED_ACTION_WIDTH: f32 = 44.0;
/// The glyph of the button that opens the ... menu.
pub(crate) const MORE_GLYPH: &str = "…";
/// The pinned actions, left to right.
pub(crate) const PINNED_ACTIONS: [TabHit; 3] = [TabHit::NewTab, TabHit::Split, TabHit::Overflow];
/// A tab never grows past this; a longer name is cut with an ellipsis so it
/// cannot push its own × out.
pub(crate) const MAX_TAB_WIDTH: f32 = 240.0;
/// The refusal while the host is suspended or speaks another version.
pub(crate) const HOST_UNAVAILABLE: &str = "Host unavailable or command pending";
/// Left padding before a tab's label.
const TAB_PADDING: f32 = metric::INSET;
// Rail.dc.html:64 and :66.
const SPACE_SWITCHER_WIDTH: f32 = 180.0;
const SPACE_PICKER_MAX_HEIGHT: f32 = 288.0;
/// Width of the switcher's ▾, measured in the host tests' font.
const SWITCHER_CHEVRON_WIDTH: f32 = 8.0;
/// The space switcher with its name cut to nothing: padding, status dot, ▾,
/// the two gaps around the empty name and the right rule.
const SPACE_SWITCHER_MIN_WIDTH: f32 = TAB_PADDING * 2.
    + metric::SMALL_DOT
    + metric::INLINE_GAP * 2.
    + SWITCHER_CHEVRON_WIDTH
    + crate::layout::RULE;
/// The room the tab strip keeps when the row is short (issue #209): one
/// tab's leading padding and its × button, so a tab stays on screen to tap
/// and scroll.
const MIN_TAB_STRIP_WIDTH: f32 = TAB_PADDING + CLOSE_WIDTH;

/// The narrowest content (the row less its leading and trailing room) that
/// holds the tab row's items without cutting the space switcher's name:
/// the switcher at its full width while it shows (`switcher`), a tab and the
/// pinned actions (+, split, ...; `pinned_width`). The frame keeps the row out of a top band whose status
/// icons would leave it less (issue #209).
pub(crate) fn tab_row_min_content(switcher: bool) -> f32 {
    let switcher = if switcher { SPACE_SWITCHER_WIDTH } else { 0.0 };
    switcher + MIN_TAB_STRIP_WIDTH + pinned_width()
}

/// The widest the space switcher may draw in a row whose content (the row
/// less its leading and trailing room) is `content` wide: what is left after
/// the pinned actions (+, split, ...) and a tab's minimum, between the switcher's minimum
/// and its 180 pt. Its name is cut to fit; +, split and ... keep their width
/// and stay inside the content (issue #209).
pub(crate) fn space_switcher_max_width(content: f32) -> f32 {
    let room = content - pinned_width() - MIN_TAB_STRIP_WIDTH;
    if room.is_nan() {
        return SPACE_SWITCHER_MIN_WIDTH;
    }
    room.clamp(SPACE_SWITCHER_MIN_WIDTH, SPACE_SWITCHER_WIDTH)
}

/// Tab row metrics (plan v2 7.5 `tab_row_metrics`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TabRowMetrics {
    pub(crate) height: f32,
    pub(crate) close_width: f32,
    pub(crate) close_height: f32,
}

/// What a tap on the tab row hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabHit {
    Select(usize),
    Close(usize),
    NewTab,
    Split,
    Overflow,
}

pub(crate) fn tab_row_metrics() -> TabRowMetrics {
    TabRowMetrics {
        height: TAB_ROW_HEIGHT,
        close_width: CLOSE_WIDTH,
        close_height: TAB_ROW_HEIGHT,
    }
}

/// Width the pinned actions take at the right edge.
pub(crate) fn pinned_width() -> f32 {
    PINNED_ACTION_WIDTH * PINNED_ACTIONS.len() as f32
}

/// Hit test for a tap at `x` given each tab's width and the row width; the
/// pinned actions (+, split, ...) stay at the right edge, or end
/// `controls_trailing` before it (L0's `ControlsClearance::tab_row_trailing`,
/// window controls on the right), as `render_tab_row` draws them.
///
/// `x` is measured from the row's left edge with the tab strip scrolled to
/// its start. Tabs that do not fit run under the pinned actions, which win
/// every tap in their area. The trailing clearance is the window controls'
/// place: the system takes those taps, and the row hits nothing there.
pub(crate) fn tab_hit(
    x: f32,
    tab_widths: &[f32],
    row_width: f32,
    controls_trailing: f32,
) -> Option<TabHit> {
    let content_right = (row_width - controls_trailing.max(0.0)).max(0.0);
    if !(0.0..content_right).contains(&x) {
        return None;
    }
    let split_left = (content_right - PINNED_ACTION_WIDTH * 2.).max(0.);
    if x >= split_left {
        let slot = ((x - split_left) / PINNED_ACTION_WIDTH).floor() as usize;
        return PINNED_ACTIONS.get(slot + 1).copied();
    }
    let tabs_width = tab_widths
        .iter()
        .map(|w| w.clamp(0., MAX_TAB_WIDTH))
        .sum::<f32>();
    let new_left = tabs_width.min((content_right - pinned_width()).max(0.));
    if (new_left..new_left + PINNED_ACTION_WIDTH).contains(&x) {
        return Some(TabHit::NewTab);
    }
    if x >= new_left {
        return None;
    }
    let mut left = 0.0;
    for (index, width) in tab_widths.iter().enumerate() {
        let width = width.clamp(0.0, MAX_TAB_WIDTH);
        let right = left + width;
        if x < right {
            let close_left = right - CLOSE_WIDTH.min(width);
            return Some(if x >= close_left {
                TabHit::Close(index)
            } else {
                TabHit::Select(index)
            });
        }
        left = right;
    }
    None
}

/// One tab of the focused space: (tab id, label, focused).
type TabEntry = (String, String, bool);

fn tab_name(snapshot: &ClientShellSnapshot, tab: &ClientShellTab) -> String {
    let pane = snapshot
        .panes
        .iter()
        .filter(|pane| pane.tab_id == tab.tab_id)
        .min_by_key(|pane| {
            (
                snapshot.focused_pane_id.as_ref() != Some(&pane.pane_id),
                !pane.focused,
            )
        });
    let agent = pane.and_then(|pane| snapshot.agents.iter().find(|a| a.pane_id == pane.pane_id));
    let name = [
        tab.custom_label.then_some(tab.label.as_str()),
        agent.and_then(|a| a.agent.as_deref()),
        agent.and_then(|a| a.display_agent.as_deref()),
        pane.and_then(|pane| pane.label.as_deref()),
        (!tab.label.trim().chars().all(|c| c.is_ascii_digit())).then_some(tab.label.as_str()),
    ]
    .into_iter()
    .flatten()
    .find(|label| !label.trim().is_empty());
    let number = tab.number.to_string();
    name.map_or_else(
        || number.clone(),
        |name| format!("{number} · {}", printable_name(name)),
    )
}

struct TabAppearance {
    fill: Option<super::style::Tint>,
    text: u32,
    weight: gpui::FontWeight,
}

fn tab_appearance(selected: bool) -> TabAppearance {
    if selected {
        TabAppearance {
            fill: Some(tint::SELECTED),
            text: color::TEXT,
            weight: weight::SEMIBOLD,
        }
    } else {
        TabAppearance {
            fill: None,
            text: color::MUTED,
            weight: weight::REGULAR,
        }
    }
}

fn status_dot(ink: u32) -> gpui::Div {
    div()
        .flex_none()
        .size(px(metric::SMALL_DOT))
        .rounded_full()
        .bg(rgb(ink))
        .debug_selector(|| "shell-tab-status-dot".into())
}

fn workspace_name(workspace: &sheprd_client::protocol::ClientShellWorkspace) -> String {
    if workspace.label.trim().is_empty() {
        format!("Space {}", workspace.number)
    } else {
        printable_name(&workspace.label)
    }
}

fn printable_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

impl Smoke {
    pub(super) fn row_tabs(&self) -> Vec<TabEntry> {
        if self.terminal.core_mode {
            return self
                .core_tabs()
                .into_iter()
                .enumerate()
                .map(|(index, tab)| {
                    let number = index + 1;
                    let label = printable_name(&tab.label);
                    let label = if label.trim().is_empty() || label == number.to_string() {
                        number.to_string()
                    } else {
                        format!("{number} · {label}")
                    };
                    (tab.id, label, tab.active)
                })
                .collect();
        }
        snapshot(&self.terminal.live)
            .map(|s| {
                s.tabs
                    .iter()
                    .filter(|t| Some(&t.workspace_id) == s.focused_workspace_id.as_ref())
                    .map(|t| (t.tab_id.clone(), tab_name(s, t), t.focused))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The tab row. `controls_leading` is
    /// `ControlsClearance::tab_row_leading`: room before the first item for a
    /// Mac's traffic lights over a rail or no sidebar (0 on an iPad).
    /// `controls_trailing` is L0's `ControlsClearance::tab_row_trailing`: room
    /// after the pinned buttons for window controls on the right edge. The
    /// row's background runs under both; `tab_hit`'s `x` starts after the
    /// leading room. `row_width` is the row's whole width: the space
    /// switcher's name is cut so that the switcher, a tab, + and the pinned
    /// buttons fit between the two rooms (issue #209).
    pub(crate) fn render_tab_row(
        &mut self,
        controls_leading: f32,
        controls_trailing: f32,
        row_width: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let tabs = self.row_tabs();
        let herdr1 = !self.terminal.core_mode;
        let switcher = if herdr1 && self.chrome_state().sidebar_hidden {
            let content = row_width - controls_leading.max(0.0) - controls_trailing.max(0.0);
            self.space_switcher(space_switcher_max_width(content), cx)
        } else {
            self.shell.shortcuts.space_picker_open = false;
            None
        };
        let show_status_dot = herdr1 && switcher.is_none();
        let catalog = (!herdr1).then(|| crate::actions::catalog(&self.terminal));
        let enabled = |command| {
            catalog.as_ref().is_none_or(|rows| {
                rows.iter()
                    .any(|row| row.info.command == command && row.available)
            })
        };
        let strip = div()
            .id("shell-tab-strip")
            .debug_selector(|| "shell-tab-strip".into())
            .flex()
            .flex_shrink(1.)
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            // A vertical swipe stays vertical: it reaches an outer list, not
            // this strip as sideways motion.
            .restrict_scroll_to_axis()
            .children(
                tabs.into_iter()
                    .enumerate()
                    .map(|(index, (id, label, focused))| {
                        self.tab_element(index, (id, label, focused), show_status_dot, cx)
                    }),
            );
        let new_tab = self.pinned_button(
            "shell-new-tab",
            "New tab",
            Icon::NewTab,
            enabled(Command::Tab),
            cx,
            |this, cx| this.tab_row_command(Command::Tab, cx),
        );
        let pinned = div()
            .debug_selector(|| "shell-tab-pinned".into())
            .flex()
            .flex_none()
            .h_full()
            .child(self.pinned_button(
                "shell-split",
                "Split right",
                Icon::Split,
                enabled(Command::SplitRight),
                cx,
                |this, cx| this.tab_row_command(Command::SplitRight, cx),
            ))
            .child(self.pinned_button(
                "shell-more",
                "More actions",
                Icon::More,
                true,
                cx,
                |this, cx| {
                    this.shell.shortcuts.space_picker_open = false;
                    this.shell.frame.overflow_open ^= true;
                    cx.notify();
                },
            ));
        Some(
            div()
                .id("shell-tab-row")
                .debug_selector(|| "shell-tab-row".into())
                .relative()
                .flex()
                .flex_none()
                .w_full()
                .h(px(TAB_ROW_HEIGHT))
                .pl(px(controls_leading))
                .pr(px(controls_trailing))
                .bg(rgb(color::SURFACE))
                .text_color(rgb(color::TEXT))
                .children(switcher)
                .child(strip)
                .children(self.render_tab_menu(cx))
                .child(new_tab)
                .child(div().flex_1())
                .child(pinned)
                .into_any_element(),
        )
    }

    fn tab_element(
        &self,
        index: usize,
        (id, label, focused): TabEntry,
        show_status_dot: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let host = self.terminal.reconnect.target.clone();
        let generation = self.terminal.link.generation;
        let boot = snapshot(&self.terminal.live)
            .map(|s| s.boot_id.clone())
            .unwrap_or_default();
        let (select_host, select_id, select_boot) = (host.clone(), id.clone(), boot.clone());
        let core_mode = self.terminal.core_mode;
        let dot = snapshot(&self.terminal.live)
            .and_then(|snapshot| snapshot.tabs.iter().find(|tab| tab.tab_id == id))
            .map(|tab| from_agent_status(tab.agent_status).dot_color())
            .unwrap_or(color::SLATE);
        let appearance = tab_appearance(focused);
        let editing = self.tab_editing(&id);
        let press = menu::TabMenu {
            host: host.clone(),
            id: id.clone(),
            boot: boot.clone(),
            generation,
            core: core_mode,
            at: Default::default(),
        };
        let (rename_host, rename_id, rename_boot) = (host.clone(), id.clone(), boot.clone());
        let name = self.tab_label(index, &label, &boot, &id, appearance.weight, focused);
        div()
            .id(("shell-tab", index))
            .debug_selector(move || format!("shell-tab-{index}"))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .max_w(px(MAX_TAB_WIDTH))
            // The field covers the name: the tab is as wide as it may get.
            .when(editing, |tab| tab.w(px(MAX_TAB_WIDTH)))
            .h_full()
            .border_r_1()
            .border_color(rgb(color::RAISED))
            .text_color(rgb(appearance.text))
            .when_some(appearance.fill, |tab, fill| tab.bg(gpui::Rgba::from(fill)))
            .when(focused, |tab| {
                tab.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(metric::CURRENT_LINE))
                        .bg(rgb(color::ACCENT))
                        .debug_selector(|| "shell-tab-underline".into()),
                )
            })
            .child(
                div()
                    .id(("shell-tab-body", index))
                    .role(Role::Tab)
                    .aria_label(label.clone())
                    .flex()
                    .flex_1()
                    .h_full()
                    .items_center()
                    .gap(px(metric::INLINE_GAP))
                    .min_w_0()
                    .pl(px(TAB_PADDING))
                    .when(focused && show_status_dot, |body| {
                        body.child(
                            status_dot(dot)
                                .debug_selector(|| "shell-selected-tab-status-dot".into()),
                        )
                    })
                    .child(name)
                    .child(self.tab_press_layer(press, cx))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if this.terminal.core_mode != core_mode {
                                return;
                            }
                            // The first tap moved to the tab; the second renames it.
                            if event.click_count() == 2 {
                                this.tab_double_tap(
                                    &rename_host,
                                    &rename_id,
                                    &rename_boot,
                                    generation,
                                );
                                cx.notify();
                                return;
                            }
                            let result = if core_mode {
                                this.core_tab_action(&select_host, generation, &select_id, false)
                            } else {
                                this.focus_tab(&select_host, &select_id, &select_boot, generation)
                            };
                            if let Err(error) = result {
                                this.terminal.live.status = error.to_string();
                            }
                            window.dismiss_virtual_keyboard();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id(("shell-tab-close", index))
                    .debug_selector(move || format!("shell-tab-close-{index}"))
                    .role(Role::Button)
                    .aria_label(format!("Close tab {label}"))
                    .flex_none()
                    .w(px(CLOSE_WIDTH))
                    .h(px(TAB_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(color::MUTED))
                    .child("×")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.terminal.core_mode != core_mode {
                            return;
                        }
                        let result = if core_mode {
                            this.core_tab_action(&host, generation, &id, true)
                        } else {
                            this.close_tab_by_id(&host, &id, &boot, generation)
                        };
                        if let Err(error) = result {
                            this.terminal.live.status = error.to_string();
                        }
                        window.dismiss_virtual_keyboard();
                        cx.stop_propagation();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn pinned_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: Icon,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let more_open = id == "shell-more" && self.shell.frame.overflow_open;
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .debug_selector(move || id.into())
            .flex_none()
            .w(px(PINNED_ACTION_WIDTH))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .when(more_open, |button| button.bg(rgb(color::RAISED)))
            .child(icon.element(if more_open {
                color::TEXT
            } else if enabled {
                color::MUTED
            } else {
                color::DISABLED
            }))
            .when(enabled, |view| {
                view.on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            })
            .children(
                (id == "shell-more")
                    .then(|| crate::qa_ax::mark(crate::qa_ax::ids::TABS_MORE, MORE_GLYPH))
                    .flatten(),
            )
            .into_any_element()
    }

    /// The space switcher, at most `max_width` wide (`space_switcher_max_width`).
    fn space_switcher(&self, max_width: f32, cx: &mut Context<Self>) -> Option<AnyElement> {
        let shown = snapshot(&self.terminal.live)?;
        let id = shown.focused_workspace_id.as_deref()?;
        let workspace = shown.workspaces.iter().find(|w| w.workspace_id == id)?;
        let label = self.shown_name(
            &shown.boot_id,
            &RenameObject::Space(workspace.workspace_id.clone()),
            workspace_name(workspace),
        );
        let dot = from_agent_status(workspace.agent_status).dot_color();
        Some(
            div()
                .id("shell-space-switcher")
                .debug_selector(|| "shell-space-switcher".into())
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(metric::INLINE_GAP))
                .h_full()
                .max_w(px(max_width))
                .px(px(TAB_PADDING))
                .border_r_1()
                .border_color(rgb(color::RAISED))
                .role(Role::Button)
                .aria_label(format!("Switch space, {label}"))
                .child(status_dot(dot))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(weight::SEMIBOLD)
                        .child(label),
                )
                .child(div().flex_none().text_color(rgb(color::MUTED)).child("▾"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.shell.frame.overflow_open = false;
                    this.shell.shortcuts.space_picker_open ^= true;
                    cx.notify();
                }))
                .when(self.shell.shortcuts.space_picker_open, |button| {
                    button.child(deferred(
                        div()
                            .id("shell-space-picker-anchor")
                            .absolute()
                            .left_0()
                            .top_0()
                            .w(px(metric::MENU_WIDTH))
                            .child(div().h(px(TAB_ROW_HEIGHT)))
                            .child(self.space_picker(shown, cx))
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.shell.shortcuts.space_picker_open = false;
                                cx.notify();
                            })),
                    ))
                })
                .into_any_element(),
        )
    }

    fn space_picker(&self, shown: &ClientShellSnapshot, cx: &mut Context<Self>) -> AnyElement {
        let host = self.terminal.reconnect.target.clone();
        let generation = self.terminal.link.generation;
        div()
            .id("shell-space-picker")
            .debug_selector(|| "shell-space-picker".into())
            .w_full()
            .max_h(px(SPACE_PICKER_MAX_HEIGHT))
            .overflow_y_scroll()
            .occlude()
            .rounded(px(radius::POPOVER))
            .bg(rgb(color::RAISED))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.shell.shortcuts.space_picker_open = false;
                cx.notify();
            }))
            .children(
                shown
                    .workspaces
                    .iter()
                    .enumerate()
                    .map(|(index, workspace)| {
                        let label = self.shown_name(
                            &shown.boot_id,
                            &RenameObject::Space(workspace.workspace_id.clone()),
                            workspace_name(workspace),
                        );
                        let host = host.clone();
                        let boot = shown.boot_id.clone();
                        let id = workspace.workspace_id.clone();
                        div()
                            .id(("shell-space-picker-row", index))
                            .debug_selector(move || format!("shell-space-picker-row-{index}"))
                            .role(Role::Button)
                            .h(px(metric::MENU_ITEM))
                            .flex()
                            .items_center()
                            .gap(px(metric::INLINE_GAP))
                            .px(px(metric::SIDE))
                            .child(status_dot(
                                from_agent_status(workspace.agent_status).dot_color(),
                            ))
                            .child(div().min_w_0().truncate().child(label))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.shell.shortcuts.space_picker_open = false;
                                if this.terminal.reconnect.target == host
                                    && let Err(error) = this.navigate_board(
                                        &host,
                                        Target::Space(id.clone()),
                                        &boot,
                                        generation,
                                    )
                                {
                                    this.terminal.live.status = error.to_string();
                                }
                                window.dismiss_virtual_keyboard();
                                cx.stop_propagation();
                                cx.notify();
                            }))
                    }),
            )
            .into_any_element()
    }

    fn tab_row_command(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Err(e) = self.run_action(command) {
            self.terminal.live.status = e.to_string();
        }
        cx.notify();
    }

    /// Closes the tab `id` without focusing it first (S-9: × closes and does
    /// not select). It takes the same path as `run_action`'s Close tab: the
    /// input transition (native text retirement), the Settings fence, the
    /// handoff cleanup, and the suspended / version-mismatch refusals.
    pub(crate) fn close_tab_by_id(
        &mut self,
        host: &str,
        id: &str,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        self.input_transition(|this| this.close_tab_by_id_inner(host, id, boot, generation))
    }

    fn close_tab_by_id_inner(
        &mut self,
        host: &str,
        id: &str,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        if self.input_modal() {
            return Err(crate::Error::Settings(
                "terminal action blocked by settings".into(),
            ));
        }
        self.handoff.cancel();
        self.handoff.selection = None;
        if self.terminal.reconnect.target != host || self.terminal.link.generation != generation {
            return Err(crate::Error::Ssh("Stale tab; tap again".into()));
        }
        if self.terminal.reconnect.suspended || self.terminal.live.version_mismatch.get().is_some()
        {
            return Err(crate::Error::Ssh(HOST_UNAVAILABLE.into()));
        }
        self.actions.palette = false;
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host busy".into()))?;
        let s = snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        if !live.connected
            || live.version_mismatch.get().is_some()
            || live.surface.as_ref().is_some_and(|s| s.popup.is_some())
            || terminal.navigating.is_some()
            || s.boot_id != boot
            || !s.tabs.iter().any(|t| t.tab_id == id)
            || !Method::TabClose.advertised_in(&live.methods)
        {
            return Err(crate::Error::Ssh("Tab unavailable".into()));
        }
        let revision = s.revision;
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = handle
            .request(boot, Method::TabClose, serde_json::json!({ "tab_id": id }))
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        terminal.navigating = Some(crate::board::Navigation {
            boot: boot.into(),
            revision,
            target: Target::Command,
            request: request.clone(),
        });
        terminal.input.detach();
        live.pending_request = Some(request);
        live.completed_request = None;
        live.failed_request = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tab_row_tests.rs"]
mod tests;
