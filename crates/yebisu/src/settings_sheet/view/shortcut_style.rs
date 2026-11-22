//! Settings.dc.html v24, lines 52–69.
use super::{group_heading, segment, segments};
use crate::{
    app::Smoke,
    settings::Values,
    shell::style::{color, metric, text},
};
use gpui::{Context, Div, Role, SharedString, Toggled, div, prelude::*, px, rgb};

const TITLE: &str = "Shortcut style";
const SERVER_LABEL: &str = "Use server keybindings";

// Settings.dc.html:67, switch body.
const SWITCH_WIDTH: f32 = 52.;
// Settings.dc.html:67, switch body height.
const SWITCH_HEIGHT: f32 = 32.;
// Settings.dc.html:67, switch knob.
const KNOB_SIZE: f32 = 26.;
// Settings.dc.html:67, inset of the knob on the selected side.
const KNOB_INSET: f32 = 3.;

impl Smoke {
    pub(super) fn settings_shortcut_style(
        &self,
        values: &Values,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut options = segments(TITLE);
        for (herdr, label) in [(false, "iPad"), (true, "Herdr prefix")] {
            options = options.child(
                segment(
                    format!(
                        "settings-shortcut-style-{}",
                        if herdr { "herdr" } else { "ipad" }
                    ),
                    label,
                    values.herdr_prefix == herdr,
                    enabled,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let mut values = this.settings_values();
                    values.herdr_prefix = herdr;
                    this.apply_settings(values, cx);
                })),
            );
        }
        let (prefix, detail) = self
            .actions
            .host_keys
            .prefix_row(&self.terminal, &self.setup);
        let server = values.use_server_keybindings;
        let switch = div()
            .id("settings-server-keybindings")
            .debug_selector(|| "settings-server-keybindings".into())
            .role(Role::Switch)
            .aria_label(SERVER_LABEL)
            .aria_toggled(if server {
                Toggled::True
            } else {
                Toggled::False
            })
            .when(!enabled, |control| {
                control.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
            })
            .w(px(SWITCH_WIDTH))
            .h(px(SWITCH_HEIGHT))
            .rounded_full()
            .bg(rgb(if server { color::ACCENT } else { color::RAISED }))
            .px(px(KNOB_INSET))
            .flex()
            .flex_shrink_0()
            .items_center()
            .when(server, |control| control.justify_end())
            .child(div().size(px(KNOB_SIZE)).rounded_full().bg(rgb(if server {
                color::BACKGROUND
            } else {
                color::MUTED
            })))
            .on_click(cx.listener(|this, _, _, cx| {
                let mut values = this.settings_values();
                values.use_server_keybindings = !values.use_server_keybindings;
                this.apply_settings(values, cx);
            }));
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(group_heading("SHORTCUT STYLE").pb_1())
            .child(
                shortcut_row(
                    TITLE,
                    "Shortcuts, the menus and Expand sidebar show this style's keys. iPad: the prefix key goes straight to the terminal and ⌘ shortcuts do the rest. Herdr prefix: the prefix arms Herdr's chords; press it twice to send it to the terminal. App ⌘ shortcuts are off except ⌘, (Settings); ⌘C and ⌘V still copy and paste.",
                )
                .border_b_1()
                .border_color(rgb(color::RAISED))
                .child(options),
            )
            .child(
                shortcut_row("Prefix key", detail)
                    .border_b_1()
                    .border_color(rgb(color::RAISED))
                    .child(
                        div().flex_shrink_0()
                            .font_family("Menlo")
                            .px_1()
                            .child(prefix),
                    ),
            )
            .child(
                shortcut_row(
                    SERVER_LABEL,
                    "With Herdr prefix, use the chords in the host's Herdr config (for example prefix c for a new tab).",
                )
                .child(switch),
            )
    }
}

fn shortcut_row(title: &'static str, detail: impl Into<SharedString>) -> Div {
    div()
        .min_h(px(metric::SETTINGS_ROW))
        .flex()
        .items_center()
        .gap(px(metric::SIDE))
        .child(
            div().flex_1().min_w_0().child(title).child(
                div()
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(detail.into()),
            ),
        )
}
