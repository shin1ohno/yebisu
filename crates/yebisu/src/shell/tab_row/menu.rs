//! A tab's name and its long-press menu (issue #143): "Rename…" and
//! "Close Tab". The field covers the tab's name after its number ("2 · "),
//! and while it is up the tab is as wide as a tab may get. On Herdr 2 the
//! menu lists Rename disabled, with the reason under it.
use crate::{
    app::Smoke,
    shell::{
        rename::{HERDR2, RenameAnchor, RenameObject, RenameTarget},
        sidebar::long_press::row_press_layer,
        style::{color, metric, radius, shadow, text},
    },
};
use gpui::{
    AnyElement, Context, FontWeight, Pixels, Point, Role, anchored, deferred, div, prelude::*, px,
    rgb,
};

/// The menu's width (canvas 34).
const MENU_WIDTH: f32 = 224.;

/// An open tab menu: the tab and the connection it was pressed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TabMenu {
    pub(crate) host: String,
    pub(crate) id: String,
    pub(crate) boot: String,
    pub(crate) generation: u64,
    pub(crate) core: bool,
    pub(crate) at: Point<Pixels>,
}

/// "2 · build" as its number part and its name ("2 · ", "build").
pub(crate) fn split_label(label: &str) -> (&str, &str) {
    match label.find(" · ") {
        Some(at) => label.split_at(at + " · ".len()),
        None => (label, ""),
    }
}

impl Smoke {
    /// A tab's label: its number and its name, the name renamable over
    /// `TabLabel`. `focused` is the row's own flag (Herdr 1 and Herdr 2).
    pub(super) fn tab_label(
        &self,
        index: usize,
        label: &str,
        boot: &str,
        id: &str,
        font_weight: FontWeight,
        focused: bool,
    ) -> AnyElement {
        let object = RenameObject::Tab(id.to_owned());
        let plain = self.rename_label(boot, &object).is_none()
            && !self.shell.rename.editing(RenameAnchor::TabLabel, &object);
        let body = div()
            .debug_selector(move || format!("shell-tab-body-{index}"))
            .relative()
            .min_w_0()
            .font_weight(font_weight)
            .when(focused, |body| {
                body.children(crate::qa_ax::mark(
                    crate::qa_ax::ids::TABS_CURRENT,
                    "Current tab",
                ))
            });
        if plain {
            return body.truncate().child(label.to_owned()).into_any_element();
        }
        let (number, name) = split_label(label);
        let number = if name.is_empty() {
            format!("{number} · ")
        } else {
            number.to_owned()
        };
        body.flex()
            .flex_1()
            .child(div().flex_none().child(number))
            .child(self.renamable(
                div().flex_1().min_w_0().truncate(),
                boot,
                RenameAnchor::TabLabel,
                object,
                name.to_owned(),
            ))
            .into_any_element()
    }

    /// Whether the tab `id` is being renamed (it widens to `MAX_TAB_WIDTH`).
    pub(super) fn tab_editing(&self, id: &str) -> bool {
        self.shell
            .rename
            .editing(RenameAnchor::TabLabel, &RenameObject::Tab(id.to_owned()))
    }

    /// A second tap on a tab's label renames it (the first moved there).
    /// On Herdr 2 `begin_rename` refuses it with the reason.
    pub(super) fn tab_double_tap(&mut self, host: &str, id: &str, boot: &str, generation: u64) {
        let target = RenameTarget {
            host: host.to_owned(),
            kind: self.terminal.herdr_kind(),
            boot: boot.to_owned(),
            generation,
            object: RenameObject::Tab(id.to_owned()),
        };
        self.begin_rename(target, RenameAnchor::TabLabel);
    }

    /// The layer that opens the tab's menu on a long press.
    pub(super) fn tab_press_layer(
        &self,
        menu: TabMenu,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        row_press_layer(cx, move |app, at| {
            app.shell.rename.tab_menu = Some(TabMenu { at, ..menu.clone() });
        })
    }

    /// The tab menu's rename target and why Rename… is disabled, if it is
    /// (the menu and `qa.state.rename.tab_menu` both read it).
    pub(crate) fn tab_menu_rename(&self, menu: &TabMenu) -> (RenameTarget, Option<&'static str>) {
        let target = RenameTarget {
            host: menu.host.clone(),
            kind: self.terminal.herdr_kind(),
            boot: menu.boot.clone(),
            generation: menu.generation,
            object: RenameObject::Tab(menu.id.clone()),
        };
        let reason = if menu.core {
            Some(HERDR2)
        } else {
            self.rename_unavailable(&target)
        };
        (target, reason)
    }

    /// The open tab menu, drawn over everything at the press.
    pub(super) fn render_tab_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.shell.rename.tab_menu.clone()?;
        if self.input_modal() || menu.core != self.terminal.core_mode {
            self.shell.rename.tab_menu = None;
            return None;
        }
        let (target, reason) = self.tab_menu_rename(&menu);
        let item = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .role(Role::MenuItem)
                .aria_label(label)
                .flex()
                .flex_col()
                .justify_center()
                .min_h(px(metric::MENU_ITEM))
                .px(px(metric::SIDE))
                .child(label)
        };
        let close = menu.clone();
        let list = div()
            .id("shell-tab-menu")
            .debug_selector(|| "shell-tab-menu".into())
            .role(Role::Menu)
            .w(px(MENU_WIDTH))
            .py(px(metric::MENU_PADDING))
            .rounded(px(radius::POPOVER))
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.shell.rename.tab_menu = None;
                cx.notify();
            }))
            .child(
                item("shell-tab-menu-rename", "Rename…")
                    .text_color(rgb(if reason.is_some() {
                        color::DISABLED
                    } else {
                        color::TEXT
                    }))
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::TAB_MENU_RENAME,
                        "Rename tab",
                    ))
                    .children(reason.map(|why| {
                        div()
                            .text_size(px(text::SMALL))
                            .text_color(rgb(color::DISABLED))
                            .child(why)
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shell.rename.tab_menu = None;
                        match reason {
                            Some(why) => this.terminal.live.status = why.into(),
                            None => this.begin_rename(target.clone(), RenameAnchor::TabLabel),
                        }
                        cx.notify();
                    })),
            )
            .child(
                item("shell-tab-menu-close", "Close Tab")
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::TAB_MENU_CLOSE,
                        "Close tab",
                    ))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.shell.rename.tab_menu = None;
                        let result = if close.core {
                            this.core_tab_action(&close.host, close.generation, &close.id, true)
                        } else {
                            this.close_tab_by_id(
                                &close.host,
                                &close.id,
                                &close.boot,
                                close.generation,
                            )
                        };
                        if let Err(error) = result {
                            this.terminal.live.status = error.to_string();
                        }
                        window.dismiss_virtual_keyboard();
                        cx.notify();
                    })),
            );
        Some(
            deferred(anchored().position(menu.at).snap_to_window().child(list))
                .with_priority(1)
                .into_any_element(),
        )
    }
}
