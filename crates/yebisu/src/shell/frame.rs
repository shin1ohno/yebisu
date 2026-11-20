//! L2a: the two-column frame. Owner: L2a (docs/layout-contracts.md).
//!
//! Left: the sidebar column (header with the toggle, L3's list or the legacy
//! board, L2b's footer; L4's rail when L0 collapses it). Right: the terminal
//! column (tab row, find bar, terminal, L5 edit bar, L2b QA strip, status
//! bar). L0 `layout::compute` decides every size; the frame only places the
//! slots in that order. The legacy toolbar and bands stay inside the
//! terminal body until L2b retires them (plan v2 5.1 rule 1).
use super::{ChromeState, HerdrKind, fit, rail::SidebarMode};
use crate::{
    app::Smoke,
    layout::{self, ControlsReach, Insets, Layout, RULE, Rect, Size},
};
use gpui::{AnyElement, Context, Window, div, prelude::*, px, rgb};

/// Sidebar header text and its toggle (plan v2 5.2: SHD).
pub(crate) const HIDE_SIDEBAR: &str = "Hide sidebar";

/// L2a's view state.
#[derive(Debug, Default)]
pub(crate) struct FrameState {
    /// The tab row's ... menu is open.
    pub(crate) overflow_open: bool,
    /// L0 collapsed the sidebar in the last frame. `board_in_band` reads it
    /// before the body is built, so the board moves in the same frame as the
    /// body; only a resize across the collapse width takes one more frame.
    pub(crate) sidebar_collapsed: bool,
    /// The window-bounds observer that keeps `sidebar_collapsed` current
    /// before each resize redraw is registered.
    pub(crate) observing_bounds: bool,
    /// Withdrawn mod regions in the last layout (L9's Mods item).
    pub(crate) withdrawn: usize,
}

/// Where the frame puts each slot, from L0's layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrameGeometry {
    /// `None` while the sidebar column draws nothing.
    pub(crate) sidebar: Option<Rect>,
    pub(crate) tab_row: Rect,
    pub(crate) find_bar: Rect,
    /// The rect the terminal receives; `None` when L0 says too small.
    pub(crate) terminal: Option<Rect>,
    pub(crate) qa_strip: Rect,
    pub(crate) status_bar: Rect,
}

/// L0's input for the frame: the window, its safe area and keyboard, and the
/// find bar. Mod regions are empty until the mod runtime lands.
pub(crate) fn frame_input(window: Size, insets: Insets, find_bar_open: bool) -> layout::Input {
    layout::Input {
        window,
        insets,
        find_bar_open,
        pane_count: 1,
        ..layout::Input::default()
    }
}

/// The frame's rects. With the sidebar column empty (hidden by the user, or
/// collapsed with no rail yet) the terminal column takes its width too.
pub(crate) fn frame_geometry(layout: &Layout, sidebar_drawn: bool) -> FrameGeometry {
    let shift = if sidebar_drawn {
        0.0
    } else {
        (layout.tab_row.x - layout.sidebar.x).max(0.0)
    };
    let widen = |rect: Rect| Rect::new(rect.x - shift, rect.y, rect.width + shift, rect.height);
    FrameGeometry {
        sidebar: sidebar_drawn.then_some(layout.sidebar),
        tab_row: widen(layout.tab_row),
        find_bar: widen(layout.find_bar),
        terminal: layout.terminal.rect().map(widen),
        qa_strip: widen(layout.qa_strip),
        status_bar: widen(layout.status_bar),
    }
}

/// The terminal column's slots, top to bottom (find bar at the top, q8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColumnSlot {
    TabRow,
    FindBar,
    Terminal,
    EditBar,
    QaStrip,
    StatusBar,
}

pub(crate) const COLUMN_ORDER: [ColumnSlot; 6] = [
    ColumnSlot::TabRow,
    ColumnSlot::FindBar,
    ColumnSlot::Terminal,
    ColumnSlot::EditBar,
    ColumnSlot::QaStrip,
    ColumnSlot::StatusBar,
];

fn rule() -> gpui::Div {
    div().flex_none().w_full().h(px(RULE)).bg(rgb(0x2b2933))
}

impl Smoke {
    /// The chrome state the entry-point model and the status bar read.
    pub(crate) fn chrome_state(&self) -> ChromeState {
        let core = self.terminal.core_mode;
        ChromeState {
            kind: if core {
                HerdrKind::Herdr2
            } else {
                HerdrKind::Herdr1
            },
            connected: self.terminal.live.connected || (core && self.terminal.core.is_some()),
            selecting: self.handoff.select_mode || self.handoff.selection.is_some(),
            sheet_open: self.settings_modal,
            find_available: crate::find_view::available(&self.terminal),
            // The rail is on screen when the user chose it or the width forces it.
            sidebar_hidden: self.actions.board_hidden || self.shell.frame.sidebar_collapsed,
            sidebar_forced: self.sidebar_forced_rail(),
        }
    }

    pub(super) fn frame_layout(&self, window: &Window) -> Layout {
        let viewport = window.viewport_size();
        let size = Size::new(f32::from(viewport.width), f32::from(viewport.height));
        self.layout_for(
            size,
            &self.platform_insets(window),
            self.platform_window_controls(window),
        )
    }

    /// `frame_layout` for a window `size` with the platform's `platform`
    /// insets (safe area and keyboard) and the reach of its window
    /// `controls` past the safe area (all zero without controls).
    pub(crate) fn layout_for(
        &self,
        size: Size,
        platform: &gpui::WindowInsets,
        controls: ControlsReach,
    ) -> Layout {
        let safe = platform.safe_area;
        let insets = Insets {
            safe_top: f32::from(safe.top),
            safe_left: f32::from(safe.left),
            safe_right: f32::from(safe.right),
            safe_bottom: f32::from(safe.bottom),
            // L9: only a keyboard docked along the bottom edge is an inset; a
            // floating one (or a smaller visual viewport) never resizes the
            // terminal.
            keyboard: fit::docked_keyboard_height(size, fit::ime_keyboard(size, &platform.ime)),
            window_controls: controls,
        };
        layout::compute(&layout::Input {
            // Command-B and the rail's button collapse to 72 at any width (L4).
            sidebar_rail: self.sidebar_user_mode() == SidebarMode::Rail,
            // L9: the requested mod regions, size 0 once the line has been
            // down for 30 s.
            regions: self.requested_mod_regions(self.fit_now()),
            ..frame_input(size, insets, self.find.is_some())
        })
    }

    /// Wraps the terminal screen built by `render_terminal` in the frame:
    /// sidebar column (L3 list or the board, L2b footer, L4 rail), tab row,
    /// find bar, terminal column, L5 edit bar, L2b QA strip, status bar.
    pub(crate) fn shell_frame(
        &mut self,
        body: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layout = self.frame_layout(window);
        self.shell.frame.withdrawn = layout.withdrawn.len();
        self.record_window_controls(self.platform_window_controls(window), layout.controls);
        self.observe_frame_bounds(window, cx);
        let board_in_band = self.board_in_band();
        if self.shell.frame.sidebar_collapsed != layout.sidebar_collapsed {
            self.shell.frame.sidebar_collapsed = layout.sidebar_collapsed;
            // A notify inside render does not draw again by itself; defer it
            // so the corrected frame follows in the same effect cycle.
            cx.defer_in(window, |_, _, cx| cx.notify());
        }
        let sidebar = self.render_sidebar_column(&layout, board_in_band, cx);
        let geometry = frame_geometry(&layout, sidebar.is_some());
        let mut body = Some(body);
        // The bar fits the terminal column it floats over (L5 N-3).
        self.set_edit_bar_width(Some(geometry.tab_row.width));
        let mut column = div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .h_full()
            .w(px(geometry.tab_row.width))
            .min_w_0();
        for slot in COLUMN_ORDER {
            column = match slot {
                ColumnSlot::TabRow => column
                    .children(self.render_tab_row(layout.controls.tab_row_trailing, cx))
                    .child(rule()),
                ColumnSlot::FindBar => column.children(self.render_find_bar(cx)),
                // The edit bar is absolute (edit_bar.rs): placed inside the
                // relative terminal slot it floats over the terminal's top
                // edge, never over the tab row above it (review SF-1).
                ColumnSlot::Terminal => column.child(
                    div()
                        .id("shell-terminal")
                        .debug_selector(|| "shell-terminal".into())
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .children(body.take())
                        .children(self.render_edit_bar(cx)),
                ),
                ColumnSlot::EditBar => column,
                ColumnSlot::QaStrip => column.children(self.render_qa_strip(cx)),
                ColumnSlot::StatusBar => column
                    .child(rule())
                    .children(self.render_status_bar(window, cx)),
            };
        }
        let column = column
            .children(self.render_overflow_backdrop(cx))
            .children(self.render_overflow_menu(cx));
        div()
            .id("shell-frame")
            .size_full()
            .flex()
            .flex_row()
            .pt(px(layout.tab_row.y))
            .pl(px(layout.sidebar.x))
            .pr(px((f32::from(window.viewport_size().width)
                - layout.tab_row.right())
            .max(0.0)))
            .pb(px(window_bottom_inset(&layout, window)))
            .bg(rgb(0x101419))
            .text_color(rgb(0xd8dee9))
            .when_some(sidebar, |view, sidebar| {
                view.child(sidebar)
                    .child(div().flex_none().h_full().w(px(RULE)).bg(rgb(0x2b2933)))
            })
            .child(column)
            .into_any_element()
    }

    /// Keeps `sidebar_collapsed` current when the window resizes, before the
    /// redraw, so `board_in_band` is right in the first frame at a new size.
    fn observe_frame_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shell.frame.observing_bounds {
            return;
        }
        self.shell.frame.observing_bounds = true;
        cx.observe_window_bounds(window, |this, window, cx| {
            let collapsed = this.frame_layout(window).sidebar_collapsed;
            if this.shell.frame.sidebar_collapsed != collapsed {
                this.shell.frame.sidebar_collapsed = collapsed;
                cx.notify();
            }
        })
        .detach();
    }

    /// The sidebar column, or `None` when it draws nothing.
    fn render_sidebar_column(
        &mut self,
        layout: &Layout,
        board_in_band: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if layout.sidebar_collapsed {
            return self.render_rail(layout.controls.rail_top, cx).map(|rail| {
                div()
                    .flex_none()
                    .h_full()
                    .w(px(layout.sidebar.width))
                    .child(rail)
                    .into_any_element()
            });
        }
        let list = match self.render_sidebar_list(cx) {
            Some(list) => Some(list),
            // Herdr 2: L8's sessions (herdr2_frame::sidebar), once a Herdr 2
            // host has listed them. With no such host the list stays empty,
            // but the column still draws: the L2b footer is the only place of
            // Hosts, Settings and New space once the toolbar is retired
            // (Integration 2). Integration 1 S3 dropped the column while it
            // was empty; with the footer in it, it is not.
            None if self.terminal.core_mode => self.render_herdr2_sidebar_list(cx),
            // The body drew the board this frame (a resize just widened the
            // window past the collapse width): never draw it twice.
            None if board_in_band => None,
            None => Some(self.render_board(cx)),
        };
        Some(
            div()
                .id("shell-sidebar")
                .flex_none()
                .h_full()
                .w(px(layout.sidebar.width))
                .flex()
                .flex_col()
                .bg(rgb(0x16161b))
                .child(
                    div()
                        .flex_none()
                        .h(px(layout::SIDEBAR_HEADER_HEIGHT))
                        .flex()
                        .items_center()
                        // The window controls sit at the header's left.
                        .pl(px(layout.controls.header_leading))
                        .pr(px(layout::SIDEBAR_HEADER_PADDING))
                        // L4: the filter field takes the header's free width.
                        .child(self.render_filter_field())
                        .child(
                            div()
                                .id("shell-sidebar-toggle")
                                .h_full()
                                .flex()
                                .items_center()
                                .px_2()
                                .text_color(rgb(0xacb2bb))
                                .child(HIDE_SIDEBAR)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_sidebar();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .id("shell-sidebar-list")
                        .debug_selector(|| "shell-sidebar-list".into())
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(list),
                )
                .children(self.render_sidebar_footer(cx))
                .into_any_element(),
        )
    }

    /// Whether the legacy board stays in the terminal screen's band list.
    /// False: the sidebar column hosts it when expanded and the rail lists the
    /// same hosts and spaces when collapsed.
    pub(crate) fn board_in_band(&self) -> bool {
        false
    }
}

/// Space under the status bar: the larger of the keyboard and the safe area
/// bottom (D1), as L0 used it.
fn window_bottom_inset(layout: &Layout, window: &Window) -> f32 {
    let height = f32::from(window.viewport_size().height);
    (height - layout.status_bar.bottom()).max(0.0)
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "frame_overlay_tests.rs"]
mod overlay_tests;

#[cfg(test)]
#[path = "frame_controls_tests.rs"]
mod controls_tests;
