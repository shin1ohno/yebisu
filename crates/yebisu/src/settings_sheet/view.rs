//! Production shell: SH1-157 accepted sheet clamp; no proposed terminal floor.
use crate::{
    app::Smoke,
    settings::Values,
    settings_pages::{Page, fonts::Density, general::Document},
};
use gpui::{prelude::*, *};

pub(crate) fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    let id = id.into();
    let label = label.into();
    div()
        .id(ElementId::Name(id.clone()))
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .when(!enabled, |button| {
            button.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
        .min_h(px(44.))
        .px_3()
        .py_2()
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(0x2b2933))
        .rounded(px(8.))
        .bg(rgb(0x1c1c22))
        .text_color(rgb(if enabled { 0xd8dee9 } else { 0xacb2bb }))
        .child(label)
}
impl Smoke {
    pub(crate) fn settings_button(&self, cx: &mut Context<Self>) -> AnyElement {
        button("open-settings", "Settings", true)
            .on_click(cx.listener(|this, _, window, cx| this.begin_settings_modal(window, cx)))
            .into_any_element()
    }
    pub(crate) fn render_settings_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available = window.fully_visible_bounds();
        let width = (f32::from(available.size.width) - 32.).clamp(0., 1000.);
        let height = (f32::from(available.size.height) - 32.).clamp(0., 760.);
        let values = self.settings_values();
        let can_edit = self.settings.is_some();
        let mut pages = div()
            .id("settings-categories")
            .flex()
            .flex_wrap()
            .gap_2()
            .overflow_y_scroll();
        for page in Page::ALL {
            pages = pages.child(
                button(format!("settings-page-{}", page.id()), page.id(), true).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.key_ticket = None;
                        if let Some(host) = &mut this.settings {
                            host.keys.view_open();
                        }
                        this.settings_page = page;
                        cx.notify();
                    }),
                ),
            );
        }
        let content = match self.settings_page {
            Page::Fonts => self.settings_fonts(&values, can_edit, cx),
            Page::Keyboard => self.settings_keyboard(&values, can_edit, cx),
            Page::DeviceKey => self.settings_keys(cx),
            Page::General => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(Document::Notice.text())
                .child(Document::License.text())
                .into_any_element(),
        };
        let status = self
            .settings
            .as_ref()
            .map_or("Settings unavailable".into(), |host| {
                host.error.as_ref().map_or_else(
                    || {
                        if host.ready() {
                            "Saved".into()
                        } else {
                            "Saving…".into()
                        }
                    },
                    |e| format!("Not saved: {e}"),
                )
            });
        let header =
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .flex_shrink_0()
                .child("Settings")
                .child(status)
                .child(
                    button("settings-retry", "Retry save", can_edit).on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(host) = &mut this.settings {
                                host.retry();
                            }
                            cx.notify();
                        },
                    )),
                )
                .child(button("settings-close", "Close", true).on_click(
                    cx.listener(|this, _, window, cx| this.end_settings_modal(window, cx)),
                ));
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let header = header.children(self.settings_qa_controls(cx));
        let body = div()
            .flex()
            .min_h_0()
            .flex_1()
            .when(width < 760., |d| d.flex_col())
            .child(pages.when(width >= 760., |d| d.w(px(240.)).flex_col().flex_shrink_0()))
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .p_3()
                    .child(content),
            );
        div()
            .id("settings-overlay")
            .debug_selector(|| "settings-overlay".into())
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x101419b0))
            .track_focus(&self.settings_focus)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
                crate::p11_observer::callback(&this.observation, "sheet-modal", "down");
                this.actions.consumed.insert(event.keystroke.key.clone());
                cx.stop_propagation();
                window.prevent_default();
                if event.keystroke.key == "escape" && !event.is_held {
                    if this.key_ticket.take().is_some() {
                        if let Some(h) = &mut this.settings {
                            h.keys.cancel();
                        }
                        cx.notify();
                    } else {
                        this.end_settings_modal(window, cx);
                    }
                }
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
                crate::p11_observer::callback(&this.observation, "sheet-modal", "up");
                this.actions.consumed.remove(&event.keystroke.key);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .id("settings-panel")
                    .debug_selector(|| "settings-panel".into())
                    .absolute()
                    .left(available.origin.x + px((f32::from(available.size.width) - width) / 2.))
                    .top(available.origin.y + px(16.))
                    .w(px(width))
                    .h(px(height))
                    .bg(rgb(0x101419))
                    .text_color(rgb(0xd8dee9))
                    .border_1()
                    .border_color(rgb(0x2b2933))
                    .rounded(px(18.))
                    .flex()
                    .flex_col()
                    .p_3()
                    .gap_2()
                    .overflow_y_scroll()
                    .child(header)
                    .child(body),
            )
            .into_any_element()
    }
    fn settings_fonts(&self, values: &Values, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut view = div().flex().flex_col().gap_2().child("Terminal font");
        for density in Density::ALL {
            let metrics = density.metrics();
            let label = format!(
                "{:?} {}/{}",
                density, metrics.size_px, metrics.line_height_px
            );
            view = view.child(
                button(format!("density-{density:?}"), label, enabled).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.select_settings_density(density, cx);
                    },
                )),
            );
        }
        let (size, height) = values.font.metrics(values.compact);
        view = view
            .child(format!(
                "Current: {size} px / {height} line · {:?}",
                Density::from_values(values)
            ))
            .child(
                div()
                    .font_family(values.font.family.clone())
                    .text_size(px(size))
                    .child("日本語 ABC → 0123456789"),
            );
        for (id, label, delta) in [("font-minus", "Smaller", -1.), ("font-plus", "Larger", 1.)] {
            view = view.child(button(id, label, enabled).on_click(cx.listener(
                move |this, _, _, cx| {
                    let mut v = this.settings_values();
                    v.font.size = Some((v.font.metrics(v.compact).0 + delta).clamp(8., 48.));
                    this.apply_settings(v, cx);
                },
            )));
        }
        view = view.child(format!(
            "Family: {} (bundled: {})",
            values.font.family,
            crate::settings_pages::fonts::TERMINAL_FAMILY
        ));
        for (index, family) in self.settings_fonts.iter().enumerate() {
            let family = family.clone();
            view = view.child(
                button(format!("font-family-{index}"), family.clone(), enabled).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.select_settings_font_family(family.clone(), cx);
                    }),
                ),
            );
        }
        view.into_any_element()
    }
    fn settings_keyboard(&self, v: &Values, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut view = div().flex().flex_col().gap_2();
        for (id, label, value) in [
            (
                "priority",
                "Terminal shortcut priority",
                crate::settings_pages::keyboard::terminal_priority(v),
            ),
            ("board", "Hide board", v.board_hidden),
            ("drag", "Start: Drag", v.drag_select),
            ("copy", "Copy automatically", v.copy_on_select),
        ] {
            view = view.child(
                button(
                    format!("settings-{id}"),
                    format!("{label}: {value}"),
                    enabled,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let mut v = this.settings_values();
                    match id {
                        "priority" => {
                            let value = !v.terminal_priority;
                            crate::settings_pages::keyboard::set_terminal_priority(&mut v, value)
                        }
                        "board" => v.board_hidden = !v.board_hidden,
                        "drag" => v.drag_select = !v.drag_select,
                        _ => v.copy_on_select = !v.copy_on_select,
                    };
                    this.apply_settings(v, cx);
                })),
            );
        }
        for row in crate::settings_pages::keyboard::shortcut_rows(&self.terminal) {
            view = view.child(format!(
                "{} · {}{}",
                row.label,
                row.shortcuts.join(", "),
                if row.available { "" } else { " (unavailable)" }
            ));
        }
        view.into_any_element()
    }
    fn settings_keys(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(host) = &self.settings else {
            return div().child("Settings unavailable").into_any_element();
        };
        let mut view = div().flex().flex_col().gap_2().child(host.keys.label());
        if let Some(error) = host.keys.failure {
            view = view.child(format!("{error:?}"));
        }
        if !host.keys.supported() {
            return view.into_any_element();
        }
        if let Some(line) = host.keys.line() {
            view = view.child(line.to_owned());
        }
        view = view.children(self.key_qa_label());
        let idle = !host.keys.busy();
        view = view
            .child(
                button(
                    "settings-key-read",
                    "Read or initialize current device key",
                    idle,
                )
                .on_click(cx.listener(|this, _, _, cx| this.read_device_key(cx))),
            )
            .child(
                button(
                    "settings-key-copy",
                    "Copy current public key",
                    host.keys.line().is_some(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.copy_device_key(cx);
                })),
            )
            .child(
                button(
                    "settings-key-replace",
                    "Replace device key…",
                    host.keys.line().is_some(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.key_ticket = this.settings.as_mut().and_then(|h| h.keys.confirmation());
                    cx.notify();
                })),
            );
        if let Some(ticket) = self.key_ticket {
            view=view.child("Replace this installation's key on ALL hosts? Register the new key manually. Existing connections remain open.")
                .child(button("settings-key-cancel","Cancel replacement",true).on_click(cx.listener(|this,_,_,cx|{
                    this.key_ticket=None;if let Some(h)=&mut this.settings{h.keys.cancel();}cx.notify();
                })))
                .child(button("settings-key-confirm","Confirm replacement",idle).on_click(cx.listener(move|this,_,_,cx|{
                    let visible=this.settings_modal && this.settings_page==Page::DeviceKey && this.key_ticket==Some(ticket);
                    this.confirm_device_key(ticket,visible);this.key_ticket=None;cx.notify();
                })));
        }
        view.into_any_element()
    }
}
