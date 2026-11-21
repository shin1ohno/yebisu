use super::{HEADER_SPACING, TOP_BAR};
use crate::{
    app::Smoke,
    board::snapshot,
    layout::{RULE, SIDEBAR_HEADER_HEIGHT, SIDEBAR_WIDTH},
    shell::{
        sidebar::icons::Icon,
        style::{color, metric, radius, shadow, text, tint, weight},
    },
};
use gpui::{AnyElement, BoxShadow, Context, Role, div, prelude::*, px, rgb, rgba};

// CompactMenu.dc.html:64, box-shadow: 8px 0 24px rgba(0,0,0,0.4).
const SIDEBAR_SHADOW_OFFSET: f32 = 8.0;
const SIDEBAR_SHADOW_BLUR: f32 = 24.0;
const SIDEBAR_SHADOW_COLOR: u32 = 0x00000066;

impl Smoke {
    pub(super) fn render_compact_sidebar(
        &mut self,
        width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let list = self.render_sidebar_list(cx);
        let selection = self.board.selection;
        let navigation = self
            .terminal
            .navigating
            .as_ref()
            .map(|nav| nav.request.clone());
        div()
            .absolute()
            .inset_0()
            .top(px(TOP_BAR + RULE))
            .child(self.compact_backdrop(true, cx))
            .child(
                div()
                    .id("shell-compact-sidebar")
                    .debug_selector(|| "shell-compact-sidebar".into())
                    .occlude()
                    .relative()
                    .flex()
                    .flex_col()
                    .h_full()
                    .w(px(width.min(SIDEBAR_WIDTH)))
                    .border_r_1()
                    .border_color(rgb(color::RAISED))
                    .bg(rgb(color::SURFACE))
                    .shadow(vec![
                        BoxShadow::new(
                            px(SIDEBAR_SHADOW_OFFSET),
                            px(0.0),
                            rgba(SIDEBAR_SHADOW_COLOR).into(),
                        )
                        .blur_radius(px(SIDEBAR_SHADOW_BLUR)),
                    ])
                    .child(self.compact_sidebar_header(cx))
                    .child(
                        div()
                            .id("shell-compact-sidebar-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(list)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let request =
                                    this.terminal.navigating.as_ref().map(|nav| &nav.request);
                                if this.board.selection != selection
                                    || request != navigation.as_ref()
                                {
                                    this.shell.single_pane.compact.panel = None;
                                    cx.notify();
                                }
                            })),
                    )
                    .children(self.render_sidebar_footer(cx)),
            )
            .into_any_element()
    }

    fn compact_sidebar_header(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_none()
            .h(px(SIDEBAR_HEADER_HEIGHT))
            .flex()
            .items_center()
            .gap(px(HEADER_SPACING))
            .px(px(metric::INLINE_GAP))
            .child(
                Icon::Collapse
                    .button("shell-compact-close-menu", "Close menu", true)
                    .relative()
                    .children(crate::qa_ax::mark(
                        "qa.compact.drawer.close",
                        "Close compact menu",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shell.single_pane.compact.panel = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::NAME))
                    .font_weight(weight::SEMIBOLD)
                    .child("yebisu"),
            )
            .child(self.render_filter_field())
            .child(self.render_new_space_button(cx))
            .into_any_element()
    }

    pub(super) fn render_compact_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let host = self.terminal.reconnect.target.clone();
        let generation = self.terminal.link.generation;
        let core = self.terminal.core_mode;
        let boot = snapshot(&self.terminal.live).map(|s| s.boot_id.clone());
        let rows = self
            .row_tabs()
            .into_iter()
            .enumerate()
            .map(|(index, (id, name, active))| {
                let host = host.clone();
                let boot = boot.clone();
                div()
                    .id(("shell-compact-tab", index))
                    .debug_selector(move || format!("shell-compact-tab-{index}"))
                    .role(Role::Tab)
                    .aria_label(name.clone())
                    .aria_selected(active)
                    .h(px(metric::TOUCH))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_3()
                    .when(active, |row| row.bg(gpui::Rgba::from(tint::SELECTED)))
                    .child(div().truncate().child(name))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.terminal.core_mode != core || this.input_modal() {
                            return;
                        }
                        let result = if core {
                            this.core_tab_action(&host, generation, &id, false)
                        } else if let Some(boot) = &boot {
                            this.focus_tab(&host, &id, boot, generation)
                        } else {
                            return;
                        };
                        if let Err(error) = result {
                            this.terminal.live.status = error.to_string();
                        }
                        this.shell.single_pane.compact.panel = None;
                        window.dismiss_virtual_keyboard();
                        cx.notify();
                    }))
            });
        div()
            .absolute()
            .inset_0()
            .top(px(TOP_BAR + RULE))
            .child(self.compact_backdrop(false, cx))
            .child(
                div()
                    .id("shell-compact-tabs")
                    .debug_selector(|| "shell-compact-tabs".into())
                    .absolute()
                    .left_2()
                    .right_2()
                    .top_0()
                    .max_h_full()
                    .rounded(px(radius::POPOVER))
                    .shadow(vec![shadow::POPOVER.box_shadow()])
                    .bg(rgb(color::RAISED))
                    .occlude()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }
}
