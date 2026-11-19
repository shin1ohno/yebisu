//! Production shell: SH1-157 accepted sheet clamp; no proposed terminal floor.
use crate::{
    app::Smoke,
    settings::{SaveStatus, Values},
    settings_pages::{
        Page,
        choices::{self, Choice},
        fonts::Density,
        general::Document,
        sections::{self, Section},
        sheet_layout::sheet_frame,
    },
};
use gpui::{prelude::*, *};

/// Hidden by design until a supported installed-font picker is proven.
const FAMILY_PICKER: bool = false;
/// The sheet's Close button.
pub(crate) const CLOSE_LABEL: &str = "Close";

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
    /// Escape cancels a pending key confirmation first, then closes the sheet.
    pub(crate) fn settings_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.key_ticket.take().is_some() {
            if let Some(h) = &mut self.settings {
                h.keys.cancel();
            }
            cx.notify();
        } else {
            self.end_settings_modal(window, cx);
        }
    }
    pub(crate) fn render_settings_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available = window.fully_visible_bounds();
        let frame = sheet_frame(
            f32::from(available.size.width),
            f32::from(available.size.height),
        );
        let values = self.settings_values();
        let can_edit = self.settings.is_some();
        let selected = sections::section_for_page(self.settings_page);
        let mut pages = div()
            .id("settings-categories")
            .flex()
            .flex_wrap()
            .gap_2()
            .overflow_y_scroll();
        for (group, members) in sections::GROUPS {
            pages = pages.child(div().w_full().text_xs().child(group));
            for &section in members {
                pages = pages.child(self.settings_section_button(section, selected, cx));
            }
        }
        let content = match self.settings_page {
            Page::Fonts => self.settings_fonts(&values, can_edit, cx),
            Page::Appearance => self.render_p15_settings(cx),
            Page::Keyboard => self.settings_keyboard(cx),
            Page::Input => self.settings_choices(
                "How keys, Japanese input and selection reach the terminal. Changes apply at once.",
                &choices::INPUT,
                &values,
                can_edit,
                cx,
            ),
            Page::Connection => self.settings_choices(
                "How the app reconnects when it returns to the foreground.",
                &choices::CONNECTION,
                &values,
                can_edit,
                cx,
            ),
            Page::DeviceKey => self.settings_keys(cx),
            Page::Notifications => self.render_notification_settings(cx),
            Page::General => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(Document::Notice.text())
                .child(Document::License.text())
                .into_any_element(),
        };
        let status = self.settings.as_ref().map(|host| host.save_status());
        let retry = matches!(status, Some(SaveStatus::Failed { retry: true, .. }));
        let status = match status {
            None => "Settings unavailable".to_owned(),
            Some(SaveStatus::Saving) => "Saving…".to_owned(),
            Some(SaveStatus::Saved) => "Saved".to_owned(),
            Some(SaveStatus::Failed { reason, .. }) => format!("Not saved: {reason}"),
        };
        let header = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .flex_shrink_0()
            .child("Settings")
            .child(status)
            .child(self.notifications.status.clone())
            .when(retry, |header| {
                header.child(
                    button("settings-retry", "Retry", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(host) = &mut this.settings {
                                host.retry();
                            }
                            cx.notify();
                        },
                    )),
                )
            })
            .child(
                button("settings-close", CLOSE_LABEL, true)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.end_settings_modal(window, cx)),
                    )
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::SETTINGS_CLOSE,
                        CLOSE_LABEL,
                    )),
            );
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let header = header.children(self.settings_qa_controls(cx));
        let body = div()
            .flex()
            .min_h_0()
            .flex_1()
            .when(frame.stacked, |d| d.flex_col())
            .child(pages.when(!frame.stacked, |d| {
                d.w(px(240.)).flex_col().flex_nowrap().flex_shrink_0()
            }))
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
                    this.settings_escape(window, cx);
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
                    .left(available.origin.x + px(frame.left))
                    .top(available.origin.y + px(frame.top))
                    .w(px(frame.width))
                    .h(px(frame.height))
                    .bg(rgb(0x101419))
                    .text_color(rgb(0xd8dee9))
                    .border_1()
                    .border_color(rgb(0x2b2933))
                    .rounded(px(18.))
                    .flex()
                    .flex_col()
                    .p_3()
                    .gap_2()
                    .child(header)
                    .child(body),
            )
            .into_any_element()
    }
    fn settings_fonts(&self, values: &Values, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut view = div().flex().flex_col().gap_2().child("Terminal font");
        for density in Density::ALL {
            let label = density.label();
            let qa_mark = match density {
                Density::Regular => {
                    crate::qa_ax::mark(crate::qa_ax::ids::SETTINGS_DENSITY_REGULAR, label)
                }
                Density::Compact => {
                    crate::qa_ax::mark(crate::qa_ax::ids::SETTINGS_DENSITY_COMPACT, label)
                }
            };
            view = view.child(
                button(format!("density-{density:?}"), label, enabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_settings_density(density, cx);
                    }))
                    .children(qa_mark),
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
        let board = if values.board_hidden {
            "Board: Hidden"
        } else {
            "Board: Shown"
        };
        view = view.child(
            button("settings-board", board, enabled).on_click(cx.listener(|this, _, _, cx| {
                let mut v = this.settings_values();
                v.board_hidden = !v.board_hidden;
                this.apply_settings(v, cx);
            })),
        );
        // The design hides the family picker until a supported installed-font picker is
        // proven: proportional faces break the cell grid. Size stays adjustable.
        if FAMILY_PICKER {
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
        }
        view.into_any_element()
    }
    /// The Shortcuts section: the read-only shortcut list. Its priority
    /// toggle is the Input page's Shortcut priority row (plan v2 7.4).
    fn settings_keyboard(&self, _cx: &mut Context<Self>) -> AnyElement {
        let mut view = div().flex().flex_col().gap_2();
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
    fn settings_section_button(
        &self,
        section: Section,
        selected: Option<Section>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let page = section.page();
        button(
            format!("settings-page-{}", page.id()),
            section.title(),
            true,
        )
        .when(selected == Some(section), |b| b.bg(rgb(0x2e3440)))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.key_ticket = None;
            if let Some(host) = &mut this.settings {
                host.keys.view_open();
            }
            this.settings_page = page;
            cx.notify();
        }))
        .children(section_mark(section))
    }
    /// Radio rows: one title, its explanation and two choices each.
    fn settings_choices(
        &self,
        intro: &'static str,
        rows: &[Choice],
        values: &Values,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut view = div().flex().flex_col().gap_3().child(intro);
        for &choice in rows {
            let current = choice.get(values);
            let mut options = div().flex().flex_wrap().gap_2();
            for (value, label) in [(false, choice.labels()[0]), (true, choice.labels()[1])] {
                let id = format!("settings-{}-{}", choice.id(), u8::from(value));
                options = options.child(
                    button(id, label, enabled)
                        .when(current == value, |b| {
                            b.bg(rgb(0x2e3440)).border_color(rgb(0x88c0d0))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_settings_choice(choice, value, cx);
                        })),
                );
            }
            // Canvas row: title and explanation left, the two choices right;
            // a narrow page wraps the choices under the text.
            view = view.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(220.))
                            .child(choice.title())
                            .child(div().text_sm().child(choice.detail())),
                    )
                    .child(options),
            );
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

/// A section button's QA accessibility mark, labelled with its title.
fn section_mark(section: Section) -> Option<AnyElement> {
    use crate::qa_ax::ids;
    let title = section.title();
    match section {
        Section::Display => crate::qa_ax::mark(ids::SETTINGS_PAGE_DISPLAY, title),
        Section::Theme => crate::qa_ax::mark(ids::SETTINGS_PAGE_THEME, title),
        Section::Input => crate::qa_ax::mark(ids::SETTINGS_PAGE_INPUT, title),
        Section::Shortcuts => crate::qa_ax::mark(ids::SETTINGS_PAGE_SHORTCUTS, title),
        Section::Connection => crate::qa_ax::mark(ids::SETTINGS_PAGE_CONNECTION, title),
        Section::Notifications => crate::qa_ax::mark(ids::SETTINGS_PAGE_NOTIFICATIONS, title),
        Section::HostsAndKeys => crate::qa_ax::mark(ids::SETTINGS_PAGE_HOSTS_AND_KEYS, title),
        Section::About => crate::qa_ax::mark(ids::SETTINGS_PAGE_ABOUT, title),
    }
}
