//! Drawing for Delete Host (#250): a Hosts row that swipes left to show its
//! Delete, the Edit sheet's Delete Host, and the confirmation. The rules are
//! in `delete.rs`.
use super::{delete::settle, view::button};
use crate::{
    app::Smoke,
    shell::{
        add_host::SheetMode,
        style::{color, metric, radius, shadow, text, tint, weight},
    },
};
use gpui::{
    AnyElement, Context, Rgba, Role, ScrollEvent, SharedString, TouchPhase, div, point, prelude::*,
    px, rgb,
};

/// Width of a row's Delete button, which is also how far the row scrolls.
pub(crate) const SWIPE_DELETE_WIDTH: f32 = 96.;
const CONFIRM_WIDTH: f32 = 400.;
/// The saved-host QA fixture (as the Open button's mark in `view.rs`).
const QA_ALPHA: &str = "qa@qa-alpha.invalid";

fn red_button(
    id: impl Into<SharedString>,
    label: &'static str,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    button(id, label, enabled, false)
        .relative()
        .when(enabled, |button| {
            button.bg(rgb(color::RED)).text_color(rgb(color::TEXT))
        })
        .when(!enabled, |button| {
            button.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
}

impl Smoke {
    /// One saved host in the Hosts list: name, Open and Edit, with Delete
    /// past the row's right edge. The row scrolls sideways only, so a
    /// vertical pan that starts on it still scrolls the list.
    pub(super) fn hosts_list_row(
        &mut self,
        index: usize,
        host: &str,
        label: String,
        controls: [AnyElement; 2],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.setup.busy;
        let swipe = self.shell.hosts_sheet.swipes.handle(host);
        let qa = host == QA_ALPHA;
        let delete = host.to_owned();
        let settled = swipe.clone();
        let [open, edit] = controls;
        div()
            .id(("hosts-row", index))
            .debug_selector(move || format!("hosts-row-{index}"))
            .w_full()
            .flex()
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .track_scroll(&swipe)
            .on_scroll(move |event: &ScrollEvent, window, cx| {
                if event.touch_phase != TouchPhase::Ended {
                    return;
                }
                // After the whole event, so this row's own scroll step for
                // it cannot undo the settle.
                let handle = settled.clone();
                window.defer(cx, move |window, _| {
                    let offset = handle.offset();
                    let target = settle(f32::from(offset.x), SWIPE_DELETE_WIDTH);
                    if f32::from(offset.x) != target {
                        handle.set_offset(point(px(target), offset.y));
                        window.refresh();
                    }
                });
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(metric::INLINE_GAP))
                    .child(div().flex_1().min_w_0().overflow_hidden().child(label))
                    .child(open)
                    .child(edit)
                    .children(
                        qa.then(|| {
                            crate::qa_ax::mark(crate::qa_ax::ids::HOSTS_ROW_QA_ALPHA, "Host row")
                        })
                        .flatten(),
                    ),
            )
            .child(
                red_button(format!("host-delete-{index}"), "Delete", !busy)
                    .ml(px(metric::INLINE_GAP))
                    .w(px(SWIPE_DELETE_WIDTH - metric::INLINE_GAP))
                    .px_0()
                    .on_click(cx.listener(move |this, _, _, cx| this.hosts_ask_delete(&delete, cx)))
                    .children(
                        qa.then(|| {
                            crate::qa_ax::mark(crate::qa_ax::ids::HOSTS_DELETE_QA_ALPHA, "Delete")
                        })
                        .flatten(),
                    ),
            )
            .into_any_element()
    }

    /// The Edit sheet's last control: Delete Host for the host it opened for.
    pub(super) fn edit_delete_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session = self.shell.hosts_sheet.add.as_ref()?;
        if session.mode != SheetMode::Edit || !self.setup.hosts.contains(&session.saved) {
            return None;
        }
        let host = session.saved.clone();
        Some(
            div()
                .flex()
                .child(
                    red_button("host-edit-delete", "Delete Host", !self.setup.busy)
                        .font_weight(weight::SEMIBOLD)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.hosts_ask_delete(&host, cx)),
                        )
                        .children(crate::qa_ax::mark(
                            crate::qa_ax::ids::HOSTS_EDIT_DELETE,
                            "Delete Host",
                        )),
                )
                .into_any_element(),
        )
    }

    /// The confirmation over the sheet; nothing is deleted until its OK.
    pub(super) fn delete_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let confirm = self.shell.hosts_sheet.delete.clone()?;
        let busy = self.setup.busy;
        let card = div()
            .id("host-delete-confirm-card")
            .debug_selector(|| "host-delete-confirm-card".into())
            .role(Role::Dialog)
            .aria_label(confirm.title())
            .w(px(CONFIRM_WIDTH))
            .max_w_full()
            .p(px(metric::SIDE))
            .flex()
            .flex_col()
            .gap(px(metric::INLINE_GAP))
            .rounded(px(radius::POPOVER))
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .child(
                div()
                    .text_size(px(text::NAME))
                    .font_weight(weight::SEMIBOLD)
                    .child(confirm.title()),
            )
            .children(confirm.lines().into_iter().map(|line| {
                div()
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(line)
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(metric::INLINE_GAP))
                    .child(
                        button("host-delete-cancel", "Cancel", true, false)
                            .relative()
                            .on_click(cx.listener(|this, _, _, cx| this.hosts_cancel_delete(cx)))
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::HOSTS_DELETE_CANCEL,
                                "Cancel delete",
                            )),
                    )
                    .child(
                        red_button("host-delete-confirm", "Delete Host", !busy)
                            .on_click(cx.listener(|this, _, _, cx| this.hosts_confirm_delete(cx)))
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::HOSTS_DELETE_CONFIRM,
                                "Delete host",
                            )),
                    ),
            );
        Some(
            div()
                .id("host-delete-confirm")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(metric::SIDE))
                .occlude()
                .bg::<Rgba>(tint::SHEET_SCRIM.into())
                .child(card)
                .into_any_element(),
        )
    }
}
