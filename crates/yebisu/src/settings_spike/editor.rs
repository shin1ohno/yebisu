//! Unmounted editor consuming the copied row/control and appearance closure.
use super::{
    appearance::Appearance,
    draft::{Draft, Field},
    strings::fl,
    ui,
};
use gpui::{prelude::*, *};

pub(super) struct SettingsWindow {
    pub draft: Draft,
    pub focus: FocusHandle,
    pub(super) font_size: f32,
}
impl SettingsWindow {
    pub fn new(draft: Draft, cx: &mut Context<Self>) -> Self {
        Self {
            draft,
            focus: cx.focus_handle(),
            font_size: 14.,
        }
    }
    pub fn sheet_ready(&self, cx: &App) -> bool {
        self.draft.ready(cx)
    }
    pub fn flush_for_dismiss(&self, cx: &mut App) {
        self.draft.flush(cx);
    }
    fn boolean_row(
        &self,
        field: Field,
        id: &'static str,
        label: &str,
        value: bool,
        look: &Appearance,
        cx: &mut Context<Self>,
    ) -> ui::Row {
        ui::row(&look.theme, id, label.to_owned()).switch(
            ui::toggle(&look.theme, format!("{id}-toggle"), value, true).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.draft.set(field, !value, cx);
                    cx.notify();
                },
            )),
        )
    }
}
impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let values = self.draft.values(cx);
        let mut look = Appearance::for_density(values.compact);
        look.size = self.font_size;
        let weak = cx.entity().downgrade();
        let draft = self.draft.clone();
        div()
            .id("settings-editor")
            .debug_selector(|| "settings-editor".into())
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_2()
            .track_focus(&self.focus)
            .text_color(rgb(look.theme.foreground))
            .child(fl!("settings-title"))
            .child(ui::section(
                &look.theme,
                &fl!("settings-fonts-section-faces"),
                None,
            ))
            .child(
                ui::row(&look.theme, "fonts-density", "Terminal font size").control(ui::segmented(
                    &look.theme,
                    "font-size",
                    &[(true, "12 px"), (false, "14 px")],
                    Some(values.compact),
                    true,
                    move |value, _, cx| draft.set(Field::Compact, value, cx),
                )),
            )
            .child(super::number::number(
                &look.theme,
                super::number::Number {
                    id: "font-size-spike",
                    value: self.font_size,
                    range: super::appearance::FONT_SIZE_RANGE,
                    unit: super::number::Unit::Pixels,
                    look: super::number::Look::Steppers,
                    enabled: true,
                },
                move |change, _, cx| {
                    if let super::number::NumberChange::Commit(value)
                    | super::number::NumberChange::Preview(value) = change
                    {
                        let _ = weak.update(cx, |this, cx| {
                            this.font_size = value;
                            cx.notify();
                        });
                    }
                },
            ))
            .child(look.specimen())
            .child(self.boolean_row(
                Field::TerminalPriority,
                "keyboard-priority",
                "Terminal shortcuts first",
                values.terminal_priority,
                &look,
                cx,
            ))
            .child(self.boolean_row(
                Field::BoardHidden,
                "sidebar-hidden",
                "Hide board",
                values.board_hidden,
                &look,
                cx,
            ))
            .child(
                ui::button(&look.theme, "settings-save", fl!("settings-save-now"), true)
                    .on_click(cx.listener(|this, _, _, cx| this.draft.flush(cx))),
            )
            .child(
                ui::button(
                    &look.theme,
                    "settings-confirm-key",
                    "Confirm key regeneration",
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| this.draft.confirm_regeneration(cx))),
            )
            .children(self.draft.error(cx))
    }
}
