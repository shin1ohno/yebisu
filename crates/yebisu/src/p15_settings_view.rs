//! P15 content mounted inside P11's production modal. Remote reads fail closed.
use crate::{
    app::Smoke,
    host_settings_availability,
    theme_picker::{Palette, SUPPORTED},
};
use gpui::{AnyElement, Context, div, prelude::*, rgb};

impl Smoke {
    pub(crate) fn host_settings_status(
        &self,
    ) -> (String, host_settings_availability::Availability) {
        (
            self.terminal.reconnect.target.clone(),
            host_settings_availability::current(self.terminal.live.connected),
        )
    }
    pub(crate) fn choose_local_theme(&mut self, palette: Palette, cx: &mut Context<Self>) {
        let values = self.settings_values();
        let Some(host) = &mut self.settings else {
            return;
        };
        host.edit(values);
        host.choose_theme(palette);
        self.local_theme = palette;
        host.flush();
        self.observation
            .record(serde_json::json!({"kind":"theme-selected", "theme":palette.key()}));
        cx.notify();
    }

    pub(crate) fn render_p15_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let (target, status) = self.host_settings_status();
        let mut content = div().flex().flex_col().gap_2().child("Terminal theme");
        if self.settings.is_some() {
            content = content.children(SUPPORTED.iter().copied().map(|palette| {
                div()
                    .id(palette.key())
                    .debug_selector(move || palette.key().to_owned())
                    .role(gpui::Role::Button)
                    .aria_label(palette.label())
                    .min_h(gpui::px(44.))
                    .p_2()
                    .bg(rgb(if palette == self.local_theme {
                        0x3b4252
                    } else {
                        0x101419
                    }))
                    .child(format!(
                        "{}{}",
                        palette.label(),
                        if palette == self.local_theme {
                            " · selected"
                        } else {
                            ""
                        }
                    ))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.choose_local_theme(palette, cx)),
                    )
            }));
        } else {
            content = content.child("Local settings unavailable");
        }
        content
            .child("Host settings · read-only")
            .child(if target.is_empty() {
                "No selected host".to_owned()
            } else {
                format!("Host: {target}")
            })
            .child("File path unavailable")
            .child(
                div()
                    .id("host-settings-status")
                    .debug_selector(|| "host-settings-status".into())
                    .child(status.message()),
            )
            .child("File settings only; daemon-effective state unverified")
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "host_settings/view_tests.rs"]
mod tests;
