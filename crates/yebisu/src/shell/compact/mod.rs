//! Artboards 14 and 15: one terminal column with a slide-over sidebar.

mod pager;
mod panels;

use crate::{
    app::Smoke,
    layout::{
        self, FIND_BAR_HEIGHT, RULE, Rect, STATUS_BAR_HEIGHT, Size, TERMINAL_MIN_HEIGHT,
        TERMINAL_MIN_WIDTH, TerminalFit, TooSmall,
    },
    shell::{
        fit::{docked_keyboard_height, ime_keyboard},
        sidebar::icons::Icon,
        status_bar::{kind_label, status_bar_model},
        style::{color, metric, radius, text, weight},
        tab_row::icons::Icon as TabIcon,
    },
};
use gpui::{AnyElement, Context, MouseButton, Role, Window, div, prelude::*, px, rgb};

// Compact.dc.html:23, :25 and :53. The rule is drawn separately.
const TOP_BAR: f32 = 52.0;
const HEADER_SPACING: f32 = 4.0;
const SWITCHER_GAP: f32 = 6.0;
const KEY_ROW: f32 = 56.0;
// Compact.dc.html:37.
const TERMINAL_LINE_HEIGHT: f32 = 18.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Panel {
    Sidebar,
    Tabs,
}

#[derive(Debug, Default)]
pub(super) struct CompactState {
    panel: Option<Panel>,
    active: bool,
}

fn page_height(pane_count: usize) -> f32 {
    if pane_count >= 2 { metric::TOUCH } else { 0.0 }
}

#[derive(Clone, Copy, Debug)]
struct Geometry {
    width: f32,
    height: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Geometry {
    fn fit(self, pane_count: usize, find: bool) -> TerminalFit {
        let y = self.top
            + TOP_BAR
            + RULE
            + page_height(pane_count)
            + if find { FIND_BAR_HEIGHT } else { 0.0 };
        let height = (self.height - (y - self.top) - STATUS_BAR_HEIGHT - RULE - KEY_ROW).max(0.0);
        if self.width >= TERMINAL_MIN_WIDTH && height >= TERMINAL_MIN_HEIGHT {
            TerminalFit::Fits(Rect::new(self.left, y, self.width, height))
        } else {
            TerminalFit::TooSmall(TooSmall {
                available: Size::new(self.width, height),
                minimum: Size::new(TERMINAL_MIN_WIDTH, TERMINAL_MIN_HEIGHT),
            })
        }
    }
}

impl Smoke {
    pub(super) fn sync_compact_panels(&mut self, window: &Window) {
        let active = layout::compact_window(window.viewport_size().width.into());
        let compact = &mut self.shell.single_pane.compact;
        if compact.active != active {
            *compact = CompactState {
                active,
                panel: None,
            };
            self.shell.frame.overflow_open = false;
        }
    }

    pub(crate) fn terminal_font_metrics(&self, window: &Window) -> (f32, f32) {
        let font = &self.terminal.font;
        if font.size.is_none() && layout::compact_window(window.viewport_size().width.into()) {
            (text::BASE, TERMINAL_LINE_HEIGHT)
        } else {
            font.metrics(self.terminal.compact)
        }
    }

    fn compact_geometry(&self, window: &Window) -> Option<Geometry> {
        let viewport = window.viewport_size();
        let size = Size::new(viewport.width.into(), viewport.height.into());
        if !layout::compact_window(size.width) {
            return None;
        }
        let insets = self.platform_insets(window);
        let safe = insets.safe_area;
        let left = f32::from(safe.left).max(0.0);
        let right = f32::from(safe.right).max(0.0);
        let top = f32::from(safe.top).max(0.0);
        let keyboard = docked_keyboard_height(size, ime_keyboard(size, &insets.ime));
        let bottom = keyboard.max(safe.bottom.into());
        Some(Geometry {
            width: (size.width - left - right).max(0.0),
            height: (size.height - top - bottom).max(0.0),
            left,
            right,
            top,
            bottom,
        })
    }

    pub(crate) fn compact_terminal_fit(&self, window: &Window) -> Option<TerminalFit> {
        let geometry = self.compact_geometry(window)?;
        Some(geometry.fit(self.focused_pane_count(), self.find.is_some()))
    }

    pub(in crate::shell) fn render_compact_frame(
        &mut self,
        body: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(geometry) = self.compact_geometry(window) else {
            return body;
        };
        if self.input_modal() {
            self.shell.single_pane.compact.panel = None;
            self.shell.frame.overflow_open = false;
        }
        let layout = self.frame_layout(window);
        self.record_window_controls(self.platform_window_controls(window), layout.controls);
        self.set_edit_bar_width(Some(geometry.width));

        let pager = self.render_pane_pager(cx);
        let pane_count = self.focused_pane_count();
        let page = div()
            .w_full()
            .flex_none()
            .h(px(page_height(pane_count)))
            .overflow_hidden()
            .children(pager);
        let bar = self.render_compact_header(
            if layout.controls.rail_top > 0.0 {
                layout.controls.header_leading
            } else {
                HEADER_SPACING
            },
            layout.controls.tab_row_trailing.max(HEADER_SPACING),
            cx,
        );
        let status = self.render_compact_status(cx);
        let panel = match self.shell.single_pane.compact.panel {
            Some(Panel::Sidebar) => Some(self.render_compact_sidebar(geometry.width, cx)),
            Some(Panel::Tabs) => Some(self.render_compact_tabs(cx)),
            None => None,
        };
        let overflow = self.render_overflow_menu_at(Some(TOP_BAR + RULE), cx);
        div()
            .id("shell-compact-frame")
            .debug_selector(|| "shell-compact-frame".into())
            .size_full()
            .pt(px(geometry.top))
            .pl(px(geometry.left))
            .pr(px(geometry.right))
            .pb(px(geometry.bottom))
            .bg(rgb(color::BACKGROUND))
            .text_color(rgb(color::TEXT))
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(bar)
                    .child(rule())
                    .when(pane_count >= 2, |column| column.child(page))
                    .children(self.render_find_bar(cx))
                    .child(
                        div()
                            .id("shell-terminal")
                            .debug_selector(|| "shell-terminal".into())
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .child(body)
                            .children(self.render_edit_bar(cx)),
                    )
                    .children(self.render_qa_strip(cx))
                    .child(rule())
                    .child(status)
                    .child(self.render_compact_keys(cx))
                    .children(panel)
                    .children(self.render_overflow_backdrop(cx))
                    .children(overflow),
            )
            .into_any_element()
    }

    fn render_compact_header(
        &self,
        leading: f32,
        trailing: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tabs = self.row_tabs();
        let selected = tabs.iter().position(|(_, _, active)| *active);
        let name = selected.map_or_else(|| "Tabs".to_owned(), |index| tabs[index].1.clone());
        let index = selected.map_or(0, |index| index + 1);
        let count = tabs.len();
        let status = selected
            .and_then(|index| {
                crate::board::snapshot(&self.terminal.live)?
                    .tabs
                    .iter()
                    .find(|tab| tab.tab_id == tabs[index].0)
            })
            .map_or(color::SLATE, |tab| {
                crate::shell::sidebar_status::from_agent_status(tab.agent_status).dot_color()
            });
        let sidebar_open = self.shell.single_pane.compact.panel == Some(Panel::Sidebar);
        let tabs_open = self.shell.single_pane.compact.panel == Some(Panel::Tabs);
        div()
            .id("shell-compact-top-bar")
            .debug_selector(|| "shell-compact-top-bar".into())
            .w_full()
            .h(px(TOP_BAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(HEADER_SPACING))
            .pl(px(leading))
            .pr(px(trailing))
            .bg(rgb(color::SURFACE))
            .child(
                Icon::Collapse
                    .button(
                        "shell-compact-menu",
                        if sidebar_open { "Close menu" } else { "Menu" },
                        true,
                    )
                    .aria_expanded(sidebar_open)
                    .relative()
                    .children(crate::qa_ax::mark("qa.compact.menu", "Compact menu"))
                    .when(sidebar_open, |button| button.bg(rgb(color::RAISED)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_compact_panel(Panel::Sidebar);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("shell-compact-tab-switcher")
                    .debug_selector(|| "shell-compact-tab-switcher".into())
                    .role(Role::Button)
                    .aria_expanded(tabs_open)
                    .aria_label(format!("Switch tabs, {name}, tab {index} of {count}"))
                    .flex_1()
                    .min_w_0()
                    .h(px(metric::TOUCH))
                    .flex()
                    .items_center()
                    .gap(px(SWITCHER_GAP))
                    .px(px(HEADER_SPACING))
                    .rounded(px(radius::CONTROL))
                    .child(dot(status))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .font_weight(weight::SEMIBOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(text::SECONDARY))
                            .text_color(rgb(color::MUTED))
                            .child(format!("{index}/{count}")),
                    )
                    .child(Icon::Down.draw(color::MUTED))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if count != 0 {
                            this.toggle_compact_panel(Panel::Tabs);
                            cx.notify();
                        }
                    })),
            )
            .child(
                div()
                    .id("shell-compact-more-actions")
                    .debug_selector(|| "shell-compact-more-actions".into())
                    .role(Role::Button)
                    .aria_label("More actions")
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(metric::TOUCH))
                    .rounded(px(radius::CONTROL))
                    .child(TabIcon::More.muted())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shell.single_pane.compact.panel = None;
                        this.shell.frame.overflow_open ^= true;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_compact_status(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.chrome_state();
        self.observe_disconnect(state.connected, cx);
        let model = status_bar_model(&state, &self.terminal.reconnect.target);
        div()
            .id("shell-compact-status")
            .aria_label(model.accessibility_label)
            .h(px(STATUS_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .bg(rgb(color::SURFACE))
            .child(dot(if state.connected {
                color::GREEN
            } else {
                color::RED
            }))
            .child(div().min_w_0().truncate().child(format!(
                "{} · {} · {}",
                model.connection,
                kind_label(model.kind),
                model.state
            )))
            .into_any_element()
    }

    // K1's key elements replace this slot once its PR lands.
    fn render_compact_keys(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        div().flex_none().h(px(KEY_ROW)).into_any_element()
    }

    fn toggle_compact_panel(&mut self, panel: Panel) {
        self.shell.frame.overflow_open = false;
        let state = &mut self.shell.single_pane.compact;
        state.panel = (state.panel != Some(panel)).then_some(panel);
    }

    fn dismiss_compact_panel(
        &mut self,
        _: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shell.single_pane.compact.panel = None;
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn compact_backdrop(&self, scrim: bool, cx: &Context<Self>) -> AnyElement {
        div()
            .id("shell-compact-backdrop")
            .debug_selector(|| "shell-compact-backdrop".into())
            .absolute()
            .inset_0()
            .role(Role::Button)
            .aria_label("Close menu")
            .occlude()
            .when(scrim, |view| {
                view.bg(gpui::Rgba::from(crate::shell::style::tint::SHEET_SCRIM))
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_compact_panel))
            .into_any_element()
    }
}

fn rule() -> gpui::Div {
    div()
        .flex_none()
        .w_full()
        .h(px(RULE))
        .bg(rgb(color::RAISED))
}

fn dot(ink: u32) -> gpui::Div {
    div()
        .flex_none()
        .size(px(metric::SMALL_DOT))
        .rounded_full()
        .bg(rgb(ink))
}

#[cfg(test)]
mod tests;
