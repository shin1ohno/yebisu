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
        ChromeState,
        fit::{docked_keyboard_height, ime_keyboard},
        key_band::ROW_HEIGHT,
        sidebar::icons::Icon,
        status_bar::status_line,
        style::{color, metric, radius, text, weight},
        tab_row::icons::Icon as TabIcon,
    },
};
use gpui::{AnyElement, Context, MouseButton, Role, Window, div, prelude::*, px, rgb};

// Compact.dc.html:23, :25 and :53. The rule is drawn separately.
const TOP_BAR: f32 = 52.0;
const HEADER_SPACING: f32 = 4.0;
const SWITCHER_GAP: f32 = metric::TIGHT_GAP;
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
    pager: Option<pager::Pager>,
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
    fn fit(self, shows_pager: bool, find: bool) -> TerminalFit {
        let y = self.top
            + TOP_BAR
            + RULE
            + if shows_pager { metric::TOUCH } else { 0.0 }
            + if find { FIND_BAR_HEIGHT } else { 0.0 };
        let height =
            (self.height - (y - self.top) - STATUS_BAR_HEIGHT - RULE - ROW_HEIGHT).max(0.0);
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
    pub(crate) fn compact_window_at(&self, window: &Window) -> bool {
        let safe = self.platform_insets(window).safe_area;
        layout::compact_window(layout::usable_width(
            window.viewport_size().width.into(),
            safe.left.into(),
            safe.right.into(),
        ))
    }

    pub(super) fn sync_compact_panels(&mut self, window: &Window) {
        let active = self.compact_window_at(window);
        let compact = &mut self.shell.single_pane.compact;
        if compact.active != active {
            *compact = CompactState {
                active,
                ..CompactState::default()
            };
            self.shell.frame.overflow_open = false;
            self.shell.frame.key_band.clear();
        }
    }

    pub(crate) fn terminal_font_metrics(&self, window: &Window) -> (f32, f32) {
        let font = &self.terminal.font;
        if font.size.is_none() && self.compact_window_at(window) {
            (text::BASE, TERMINAL_LINE_HEIGHT)
        } else {
            font.metrics(self.terminal.compact)
        }
    }

    fn compact_geometry(&self, window: &Window) -> Option<Geometry> {
        let viewport = window.viewport_size();
        let size = Size::new(viewport.width.into(), viewport.height.into());
        let insets = self.platform_insets(window);
        let safe = insets.safe_area;
        let width = layout::usable_width(size.width, safe.left.into(), safe.right.into());
        if !layout::compact_window(width) {
            return None;
        }
        let left = f32::from(safe.left).max(0.0);
        let right = f32::from(safe.right).max(0.0);
        let top = f32::from(safe.top).max(0.0);
        let keyboard = docked_keyboard_height(size, ime_keyboard(size, &insets.ime));
        let bottom = keyboard.max(safe.bottom.into());
        Some(Geometry {
            width,
            height: (size.height - top - bottom).max(0.0),
            left,
            right,
            top,
            bottom,
        })
    }

    pub(crate) fn compact_terminal_fit(&mut self, window: &Window) -> Option<TerminalFit> {
        let geometry = self.compact_geometry(window)?;
        let pager = self.pager();
        let fit = geometry.fit(pager.is_some(), self.find.is_some());
        // A Core catalog can change on its worker while this frame is drawn.
        self.shell.single_pane.compact.pager = pager;
        Some(fit)
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
        // The top bar starts below the top safe area (`compact_geometry`),
        // clear of the status bar, so it keeps only the window controls'
        // clearance; the frame's layout adds the status icons' for a tab row
        // moved up into that band (`layout::top_band`, issue #208).
        let reach = self.platform_window_controls(window);
        let controls = reach.clearance();
        self.record_window_controls(reach, controls);
        self.set_edit_bar_width(Some(geometry.width));

        let pager = self.shell.single_pane.compact.pager.take();
        let bar = self.render_compact_header(
            if controls.rail_top > 0.0 {
                controls.header_leading
            } else {
                HEADER_SPACING
            },
            controls.tab_row_trailing.max(HEADER_SPACING),
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
                    .children(pager.map(|pager| pager.render(cx)))
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
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::COMPACT_MENU,
                        "Compact menu",
                    ))
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
        let (line, accessibility_label) = self.compact_status_labels(&state);
        div()
            .id("shell-compact-status")
            .aria_label(accessibility_label)
            .h(px(STATUS_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(metric::GAP))
            .px(px(metric::INSET))
            .bg(rgb(color::SURFACE))
            .child(dot(if state.connected {
                color::GREEN
            } else {
                color::RED
            }))
            .child(div().min_w_0().truncate().child(line))
            .into_any_element()
    }

    fn compact_status_labels(&self, state: &ChromeState) -> (String, String) {
        let model = self.current_status_bar_model(state);
        (status_line(&model), model.accessibility_label)
    }

    fn render_compact_keys(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_none()
            .h(px(ROW_HEIGHT))
            .overflow_hidden()
            .border_t_1()
            .border_color(rgb(color::RAISED))
            .bg(rgb(color::SURFACE))
            .child(crate::shell::key_band::render_key_row(self, None, cx))
            .into_any_element()
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
