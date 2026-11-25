//! Production shell: SH1-157 accepted sheet clamp; no proposed terminal floor.
use crate::{
    app::Smoke,
    settings::{SaveStatus, Values, sidebar_density::SidebarDensity},
    settings_pages::{
        Page,
        choices::{self, Choice},
        fonts::Density,
        general::Document,
        sections::{self, Section},
        sheet_layout::{HeaderInset, header_inset, sheet_frame},
    },
    shell::style::{color, metric, radius, shadow, text, tint, weight},
};
use gpui::{prelude::*, *};

mod shortcut_style;

/// Hidden by design until a supported installed-font picker is proven.
const FAMILY_PICKER: bool = false;
const DONE_LABEL: &str = "Done";

// Settings.dc.html:26–28
const HEADER_SIDE_WIDTH: f32 = 72.;
// Settings.dc.html:34
const SECTION_SIDE_PADDING: f32 = metric::INSET;
// Settings.dc.html:46
const CONTENT_PADDING: f32 = 28.;
const CONTENT_GAP: f32 = 24.;
// Settings.dc.html:57
const SEGMENT_SIDE_PADDING: f32 = 14.;

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
        .min_h(px(metric::TOUCH))
        .px(px(metric::INSET))
        .py(px(metric::SMALL_INSET))
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(color::RAISED))
        .rounded(px(radius::CONTROL))
        .bg(rgb(color::SURFACE))
        .text_color(rgb(if enabled { color::TEXT } else { color::MUTED }))
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
        let section = sections::section_for_page(self.settings_page);
        let mut pages = div()
            .id("settings-categories")
            .flex()
            .flex_col()
            .gap(px(metric::SEGMENT_INSET))
            .px(px(metric::INLINE_GAP))
            .py(px(SECTION_SIDE_PADDING))
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(self.shell.indicators.settings_categories.handle());
        for (group, members) in sections::GROUPS {
            pages = pages.child(
                group_heading(group)
                    .px(px(SECTION_SIDE_PADDING))
                    .pt(px(if group == "APPEARANCE" {
                        metric::INLINE_GAP
                    } else {
                        SECTION_SIDE_PADDING
                    }))
                    .pb_1(),
            );
            for &member in members {
                pages = pages.child(self.settings_section_button(member, section, cx));
            }
        }
        let pages = pages.child(self.shell.indicators.settings_categories.thumb());
        let intro = match self.settings_page {
            Page::Input => Some(
                "How keys, Japanese input and selection reach the terminal. Changes apply at once. “Saved” appears after the write succeeds; if it fails, the reason and Retry appear at the top.",
            ),
            Page::Connection => Some("How the app reconnects when it returns to the foreground."),
            _ => None,
        };
        let content = match self.settings_page {
            Page::Fonts => self.settings_fonts(&values, can_edit, window, cx),
            Page::Appearance => self.render_p15_settings(cx),
            Page::Sidebar => self.settings_sidebar(&values, can_edit, cx),
            Page::Keyboard => self.settings_keyboard(cx),
            Page::Input => div()
                .flex()
                .flex_col()
                .gap(px(CONTENT_GAP))
                .child(self.settings_shortcut_style(&values, can_edit, cx))
                .child(self.settings_choices(
                    &[
                        (Some("HARDWARE KEYBOARD"), choices::HARDWARE_KEYBOARD),
                        (Some("SELECTION"), choices::SELECTION),
                    ],
                    &values,
                    can_edit,
                    cx,
                ))
                .into_any_element(),
            Page::Connection => {
                self.settings_choices(&[(None, &choices::CONNECTION)], &values, can_edit, cx)
            }
            Page::DeviceKey => self.settings_keys(cx),
            Page::Notifications => self.render_notification_settings(cx),
            Page::General => div()
                .flex()
                .flex_col()
                .gap(px(metric::SIDE))
                .child(Document::Notice.text())
                .child(Document::License.text())
                .into_any_element(),
        };
        let body = div()
            .flex()
            .min_h_0()
            .flex_1()
            .when(frame.stacked, |d| d.flex_col())
            .child(if frame.stacked {
                pages
                    .max_h(px(frame.height / 3.))
                    .border_b_1()
                    .border_color(rgb(color::RAISED))
            } else {
                pages
                    .w(px(metric::SETTINGS_LIST))
                    .flex_shrink_0()
                    .border_r_1()
                    .border_color(rgb(color::RAISED))
            })
            .child(
                div()
                    .id((
                        ElementId::Name("settings-content".into()),
                        self.settings_page.id(),
                    ))
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(self.shell.indicators.settings_content.handle())
                    .p(px(CONTENT_PADDING))
                    .flex()
                    .flex_col()
                    .gap(px(CONTENT_GAP))
                    .children(section.map(|section| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .id("settings-page-title")
                                    .role(Role::Heading)
                                    .text_size(px(text::PAGE_TITLE))
                                    .line_height(px(text::PAGE_TITLE_LINE))
                                    .font_weight(weight::SEMIBOLD)
                                    .child(section.title()),
                            )
                            .children(
                                intro.map(|intro| div().text_color(rgb(color::MUTED)).child(intro)),
                            )
                    }))
                    .child(content)
                    .child(self.shell.indicators.settings_content.thumb()),
            );
        // The same clearance the shell chrome keeps for the window controls.
        let inset = header_inset(
            frame,
            f32::from(available.size.width),
            self.platform_window_controls(window).clearance(),
        );
        let (header, notices) = self.settings_header(inset, cx);
        div()
            .id("settings-overlay")
            .debug_selector(|| "settings-overlay".into())
            .absolute()
            .inset_0()
            .occlude()
            .bg::<Rgba>(tint::SHEET_SCRIM.into())
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
                div().id("settings-backdrop").absolute().inset_0().on_click(
                    cx.listener(|this, _, window, cx| this.end_settings_modal(window, cx)),
                ),
            )
            .child(
                div()
                    .id("settings-panel")
                    .debug_selector(|| "settings-panel".into())
                    .absolute()
                    .left(available.origin.x + px(frame.left))
                    .top(available.origin.y + px(frame.top))
                    .w(px(frame.width))
                    .h(px(frame.height))
                    .bg(rgb(color::SURFACE))
                    .text_color(rgb(color::TEXT))
                    .border_1()
                    .border_color(rgb(color::RAISED))
                    .rounded(px(radius::SHEET))
                    .shadow(vec![shadow::SHEET.box_shadow()])
                    .overflow_hidden()
                    .occlude()
                    .flex()
                    .flex_col()
                    .child(header)
                    .children(notices)
                    .child(body),
            )
            .into_any_element()
    }

    fn settings_header(
        &self,
        inset: HeaderInset,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Option<AnyElement>) {
        let status = self.settings.as_ref().map(|host| host.save_status());
        let (label, failure) = match status {
            None => ("Unavailable", None),
            Some(SaveStatus::Saving) => ("Saving…", None),
            Some(SaveStatus::Saved) => ("Saved", None),
            Some(SaveStatus::Failed { reason, retry }) => ("Not saved", Some((reason, retry))),
        };
        let header = div()
            .id("settings-header")
            .debug_selector(|| "settings-header".into())
            .h(px(metric::SHEET_HEADER))
            .flex_shrink_0()
            .flex()
            .items_center()
            .pl(px(inset.leading))
            .pr(px(inset.trailing))
            .border_b_1()
            .border_color(rgb(color::RAISED))
            .child(
                div()
                    .id("settings-save-state")
                    .debug_selector(|| "settings-save-state".into())
                    .role(Role::Status)
                    .w(px(HEADER_SIDE_WIDTH))
                    .flex_shrink_0()
                    .pl(px(metric::INLINE_GAP))
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(label),
            )
            .child(
                div()
                    .id("settings-title")
                    .debug_selector(|| "settings-title".into())
                    .flex_1()
                    .min_w_0()
                    .text_center()
                    .text_size(px(text::SHEET_TITLE))
                    .font_weight(weight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                div()
                    .id("settings-close")
                    .debug_selector(|| "settings-close".into())
                    .role(Role::Button)
                    .aria_label(DONE_LABEL)
                    .w(px(HEADER_SIDE_WIDTH))
                    .h(px(metric::TOUCH))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(weight::SEMIBOLD)
                    .text_color(rgb(color::ACCENT))
                    .child(DONE_LABEL)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.end_settings_modal(window, cx)),
                    )
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::SETTINGS_CLOSE,
                        DONE_LABEL,
                    )),
            );
        let mut notices = Vec::new();
        if let Some((reason, retry)) = failure {
            notices.push(
                div()
                    .id("settings-save-failure")
                    .debug_selector(|| "settings-save-failure".into())
                    .role(Role::Alert)
                    .bg(rgb(color::RAISED))
                    .flex()
                    .items_center()
                    .gap(px(metric::INLINE_GAP))
                    .p(px(metric::INLINE_GAP))
                    .child(div().flex_1().min_w_0().child(reason))
                    .when(retry, |band| {
                        band.child(
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
                    .into_any_element(),
            );
        }
        if self.settings_page == Page::Notifications && !self.notifications.status.is_empty() {
            notices.push(
                div()
                    .id("settings-notification-status")
                    .px(px(SECTION_SIDE_PADDING))
                    .role(Role::Status)
                    .child(self.notifications.status.clone())
                    .into_any_element(),
            );
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        notices.extend(self.settings_qa_controls(cx));
        let notices = (!notices.is_empty())
            .then(|| div().flex_shrink_0().children(notices).into_any_element());
        (header.into_any_element(), notices)
    }

    fn settings_fonts(
        &self,
        values: &Values,
        enabled: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut options = segments("Terminal font");
        let current = Density::from_values(values);
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
            options = options.child(
                segment(
                    format!("density-{density:?}"),
                    label,
                    current == density,
                    enabled,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_settings_density(density, cx);
                }))
                .children(qa_mark),
            );
        }
        let (size, height) = self.terminal_font_metrics(window);
        let mut view = div()
            .flex()
            .flex_col()
            .gap(px(metric::INLINE_GAP))
            .child(choice_row("Terminal font", None, options))
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
            let qa_mark = if delta < 0. {
                crate::qa_ax::mark(crate::qa_ax::ids::SETTINGS_FONT_SMALLER, label)
            } else {
                crate::qa_ax::mark(crate::qa_ax::ids::SETTINGS_FONT_LARGER, label)
            };
            view = view.child(
                button(id, label, enabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let mut v = this.settings_values();
                        v.font.size =
                            Some((this.terminal_font_metrics(window).0 + delta).clamp(8., 48.));
                        this.apply_settings(v, cx);
                    }))
                    .children(qa_mark),
            );
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
        let mut view = div().flex().flex_col().gap(px(metric::INLINE_GAP));
        for row in crate::settings_pages::keyboard::shortcut_rows(&self.terminal, &self.actions) {
            view = view.child(
                div()
                    .min_w_0()
                    .child(row.display(self.actions.herdr_prefix)),
            );
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
        .aria_selected(selected == Some(section))
        .h(px(metric::SETTINGS_ROW))
        .flex()
        .items_center()
        .px(px(SECTION_SIDE_PADDING))
        .py_0()
        .border_0()
        .bg(transparent_black())
        .when(selected == Some(section), |b| {
            b.bg::<Rgba>(tint::SELECTED.into())
                .font_weight(weight::SEMIBOLD)
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.key_ticket = None;
            if let Some(host) = &mut this.settings {
                host.keys.view_open();
            }
            this.settings_page = page;
            this.read_key_on_page_open(cx);
            cx.notify();
        }))
        .children(section_mark(section))
    }

    fn settings_sidebar(
        &self,
        values: &Values,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut options = segments("Board");
        for (density, label) in [
            (SidebarDensity::Dense, "Dense"),
            (SidebarDensity::Roomy, "Roomy"),
        ] {
            options = options.child(
                segment(
                    format!("settings-sidebar-{label}"),
                    label,
                    values.sidebar_density == density,
                    enabled,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let mut values = this.settings_values();
                    values.sidebar_density = density;
                    if let Some(host) = &mut this.settings {
                        host.edit_sidebar_density(values.clone());
                    }
                    this.apply_settings(values, cx);
                })),
            );
        }
        choice_row("Board", None, options).into_any_element()
    }

    fn settings_choices(
        &self,
        groups: &[(Option<&'static str>, &[Choice])],
        values: &Values,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut view = div().flex().flex_col().gap(px(CONTENT_GAP));
        for &(title, rows) in groups {
            let mut group = div()
                .flex()
                .flex_col()
                .gap_1()
                .children(title.map(|title| group_heading(title).pb_1()));
            for &choice in rows {
                let current = choice.get(values);
                let mut options = segments(choice.title());
                for (value, label) in choice.options() {
                    let id = format!("settings-{}-{}", choice.id(), u8::from(value));
                    options = options.child(
                        segment(id, label, current == value, enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_settings_choice(choice, value, cx);
                            }))
                            .children(choice_mark(choice, value, label)),
                    );
                }
                group = group.child(choice_row(choice.title(), Some(choice.detail()), options));
            }
            view = view.child(group);
        }
        view.into_any_element()
    }
    fn settings_keys(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(host) = &self.settings else {
            return div().child("Settings unavailable").into_any_element();
        };
        let mut view = div()
            .flex()
            .flex_col()
            .gap(px(metric::INLINE_GAP))
            .child(host.keys.label());
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
                .on_click(cx.listener(|this, _, _, cx| this.read_device_key(cx)))
                .children(crate::qa_ax::mark(
                    crate::qa_ax::ids::KEYS_READ,
                    "Read or initialize current device key",
                )),
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

fn group_heading(title: &'static str) -> Div {
    div()
        .text_size(px(text::SMALL))
        .font_weight(weight::SEMIBOLD)
        .text_color(rgb(color::MUTED))
        .child(title)
}

fn segments(label: &'static str) -> Stateful<Div> {
    div()
        .id(SharedString::from(label))
        .role(Role::RadioGroup)
        .aria_label(label)
        .flex()
        .flex_shrink_0()
        .bg(rgb(color::BACKGROUND))
        .rounded(px(radius::CONTROL))
        .p(px(metric::SEGMENT_INSET))
}

fn segment(id: String, label: &'static str, selected: bool, enabled: bool) -> Stateful<Div> {
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .role(Role::RadioButton)
        .aria_label(label)
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .when(!enabled, |button| {
            button.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
        .h(px(metric::SEGMENT))
        .px(px(SEGMENT_SIDE_PADDING))
        .flex()
        .items_center()
        .rounded(px(radius::RAIL_ITEM))
        .text_color(rgb(if selected { color::TEXT } else { color::MUTED }))
        .when(selected, |option| option.bg(rgb(color::RAISED)))
        .child(label)
}

fn choice_row(title: &'static str, detail: Option<&'static str>, options: Stateful<Div>) -> Div {
    div()
        .min_h(px(metric::SETTINGS_ROW))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(metric::SIDE))
        .border_b_1()
        .border_color(rgb(color::RAISED))
        .child(div().flex_1().child(title).children(detail.map(|detail| {
            div()
                .text_size(px(text::SECONDARY))
                .text_color(rgb(color::MUTED))
                .child(detail)
        })))
        .child(options)
}

/// The QA accessibility mark of an Input page choice that SH1-151 sets
/// (UI test redesign S4), labelled with the text the button paints.
fn choice_mark(choice: Choice, value: bool, label: &'static str) -> Option<AnyElement> {
    use crate::qa_ax::ids;
    match (choice, value) {
        (Choice::StartSelection, false) => {
            crate::qa_ax::mark(ids::SETTINGS_START_DOUBLE_TAP, label)
        }
        (Choice::StartSelection, true) => crate::qa_ax::mark(ids::SETTINGS_START_DRAG, label),
        (Choice::Copy, false) => crate::qa_ax::mark(ids::SETTINGS_COPY_MANUAL, label),
        (Choice::Copy, true) => crate::qa_ax::mark(ids::SETTINGS_COPY_ON_SELECTION, label),
        _ => None,
    }
}

/// A section button's QA accessibility mark, labelled with its title.
fn section_mark(section: Section) -> Option<AnyElement> {
    use crate::qa_ax::ids;
    let title = section.title();
    match section {
        Section::Display => crate::qa_ax::mark(ids::SETTINGS_PAGE_DISPLAY, title),
        Section::Theme => crate::qa_ax::mark(ids::SETTINGS_PAGE_THEME, title),
        Section::Sidebar => None,
        Section::Input => crate::qa_ax::mark(ids::SETTINGS_PAGE_INPUT, title),
        Section::Shortcuts => crate::qa_ax::mark(ids::SETTINGS_PAGE_SHORTCUTS, title),
        Section::Connection => crate::qa_ax::mark(ids::SETTINGS_PAGE_CONNECTION, title),
        Section::Notifications => crate::qa_ax::mark(ids::SETTINGS_PAGE_NOTIFICATIONS, title),
        Section::HostsAndKeys => crate::qa_ax::mark(ids::SETTINGS_PAGE_HOSTS_AND_KEYS, title),
        Section::About => crate::qa_ax::mark(ids::SETTINGS_PAGE_ABOUT, title),
    }
}
