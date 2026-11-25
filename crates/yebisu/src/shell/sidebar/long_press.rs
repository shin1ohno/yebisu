//! Long press of a sidebar host or space row (plan v2 7.11, Q8): the Host
//! or Space menu's items for that row. Owner: L3 (the gesture and the
//! menu); the items, their reasons and their run are L6's `host_menus`.
//!
//! The menu remembers the row it was shown for (review S4): an item runs
//! only on that row's host and space, and a row other than the selected
//! host or the focused space lists its items disabled, with the reason.
use super::{AgentIdentity, view::SidebarTap};
use crate::{
    app::Smoke,
    shell::{
        host_menus::{self, MenuEntry, MenuItemId, MenuScope, SpaceMenuItem},
        rename::{RenameAnchor, RenameObject},
        style::{color, metric, radius, shadow, text},
    },
};
use gpui::{
    AnyElement, Context, DispatchPhase, HitboxBehavior, LongPressEvent, Pixels, Point, TouchPhase,
    Window, anchored, canvas, deferred, div, prelude::*, px, rgb,
};

const MENU_WIDTH: f32 = 280.;
const ITEM_HEIGHT: f32 = 44.;

/// The row a long press landed on, with what its menu needs to name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PressedRow {
    Host(String),
    Space {
        host: String,
        id: String,
        name: String,
    },
    /// A port forward's row (issue 140): Open and Stop, or Dismiss.
    Forward {
        host: String,
        remote: u16,
    },
}

/// An open long-press menu: the row, the target it was shown for, its
/// items, and where the press was (window coordinates).
#[derive(Clone, Debug)]
pub(crate) struct LongPressMenu {
    pub(crate) row: PressedRow,
    pub(crate) shown: MenuScope,
    pub(crate) entries: Vec<MenuEntry>,
    pub(crate) at: Point<Pixels>,
}

impl Smoke {
    /// Opens the long-press menu of `row`.
    pub(crate) fn sidebar_long_press(&mut self, row: PressedRow, at: Point<Pixels>) {
        self.shell.sidebar.agent_menu = None;
        let state = self.menu_chrome_state();
        let current = self.current_menu_scope();
        let (shown, entries) = match &row {
            PressedRow::Host(host) => (
                MenuScope {
                    host: host.clone(),
                    space: None,
                },
                host_menus::host_long_press_for(&state, host, &current),
            ),
            PressedRow::Space { host, id, .. } => {
                let shown = MenuScope {
                    host: host.clone(),
                    space: Some(id.clone()),
                };
                let entries = host_menus::space_long_press_for(&state, &shown, &current);
                (shown, entries)
            }
            PressedRow::Forward { host, .. } => (
                MenuScope {
                    host: host.clone(),
                    space: None,
                },
                Vec::new(),
            ),
        };
        self.shell.sidebar.long_press = Some(LongPressMenu {
            row,
            shown,
            entries,
            at,
        });
    }

    /// Runs `item` from the open long-press menu and closes it. A disabled
    /// item, or a target that changed since the press, is refused.
    pub(crate) fn pick_long_press(
        &mut self,
        item: MenuItemId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let menu = self
            .shell
            .sidebar
            .long_press
            .take()
            .ok_or(crate::Error::NativeMenu("no long-press menu"))?;
        if !menu.entries.iter().any(|e| e.item == item && e.enabled) {
            return Err(crate::Error::NativeMenu("menu item unavailable"));
        }
        self.run_menu_item(item, &menu.shown, window, cx)
    }

    /// The open long-press menu, drawn over everything at the press.
    pub(crate) fn render_long_press_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.shell.sidebar.long_press.clone()?;
        let target = match &menu.row {
            PressedRow::Host(host) => host.clone(),
            PressedRow::Space { name, .. } => name.clone(),
            PressedRow::Forward { host, remote } => {
                return self.render_forward_menu(host, *remote, menu.at, cx);
            }
        };
        let mut list = div()
            .id("sidebar-long-press")
            .w(px(MENU_WIDTH))
            .py_1()
            .rounded(px(radius::EDIT_BAR))
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.shell.sidebar.long_press = None;
                cx.notify();
            }));
        for entry in menu.entries {
            let item = entry.item;
            list = list.child(
                div()
                    .id(gpui::ElementId::Name(
                        format!("sidebar-long-press-{}", item.name()).into(),
                    ))
                    .min_h(px(ITEM_HEIGHT))
                    .px(px(metric::INSET))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .text_color(rgb(if entry.enabled { color::TEXT } else { 0x6b7080 }))
                    .child(host_menus::long_press_label(item, &target))
                    .children(
                        (item == MenuItemId::Space(SpaceMenuItem::Rename))
                            .then(|| {
                                crate::qa_ax::mark(
                                    crate::qa_ax::ids::SIDEBAR_SPACE_RENAME,
                                    "Rename space",
                                )
                            })
                            .flatten(),
                    )
                    .children(entry.reason.map(|why| {
                        div()
                            .text_size(px(text::SECONDARY))
                            .text_color(rgb(0x9ca3af))
                            .child(why)
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Err(e) = this.pick_long_press(item, window, cx) {
                            this.terminal.live.status = e.to_string();
                        }
                        cx.notify();
                    }))
                    .when(
                        item == MenuItemId::Host(host_menus::HostMenuItem::ForwardPort),
                        |row| {
                            row.relative().children(crate::qa_ax::mark(
                                crate::qa_ax::ids::HOST_MENU_FORWARD,
                                host_menus::HostMenuItem::ForwardPort.label(),
                            ))
                        },
                    ),
            );
        }
        if let PressedRow::Host(host) = &menu.row {
            list = list.children(self.host_menu_forwards(host, cx));
        }
        Some(
            deferred(anchored().position(menu.at).snap_to_window().child(list))
                .with_priority(1)
                .into_any_element(),
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct AgentMenu {
    pub(super) id: AgentIdentity,
    pub(super) tap: SidebarTap,
    pub(super) at: Point<Pixels>,
}

// SidebarSpec.dc.html:42, agent actions popover; a forward row's menu too.
pub(super) const AGENT_MENU_WIDTH: f32 = 224.;

impl Smoke {
    pub(super) fn render_agent_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.shell.sidebar.agent_menu.clone()?;
        let SidebarTap::Agent {
            boot, generation, ..
        } = &menu.tap
        else {
            return None;
        };
        let pinned = self.shell.sidebar.is_agent_pinned(&menu.id, boot);
        let rename = self.rename_target(RenameObject::Pane(menu.id.pane.clone()));
        let unavailable = rename
            .as_ref()
            .map_or(Some(crate::shell::rename::NOT_CONNECTED), |target| {
                self.rename_unavailable(target)
            });
        let tap = menu.tap.clone();
        let boot = boot.clone();
        let generation = *generation;
        let id = menu.id;
        let button = |id, label| {
            div()
                .id(id)
                .role(gpui::Role::Button)
                .aria_label(label)
                .flex()
                .items_center()
                .h(px(metric::MENU_ITEM))
                .px(px(metric::SIDE))
                .child(label)
        };
        let list = div()
            .id("sidebar-agent-menu")
            .w(px(AGENT_MENU_WIDTH))
            .py(px(metric::MENU_PADDING))
            .rounded(px(radius::POPOVER))
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.shell.sidebar.agent_menu = None;
                cx.notify();
            }))
            .child(button("sidebar-agent-open", "Open").on_click(cx.listener(
                move |this, _, window, cx| {
                    this.shell.sidebar.agent_menu = None;
                    this.tap_row(tap.clone(), window, cx);
                },
            )))
            .child(
                button(
                    "sidebar-agent-pin",
                    if pinned { "Unpin Agent" } else { "Pin Agent" },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.shell.sidebar.agent_menu = None;
                    if let Err(error) =
                        this.toggle_sidebar_agent_pin(id.clone(), &boot, generation, cx)
                    {
                        this.terminal.live.status = error.to_string();
                    }
                    cx.notify();
                })),
            )
            .child(
                button("sidebar-agent-rename", "Rename…")
                    .text_color(rgb(if unavailable.is_some() {
                        color::DISABLED
                    } else {
                        color::TEXT
                    }))
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::SIDEBAR_AGENT_RENAME,
                        "Rename agent",
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shell.sidebar.agent_menu = None;
                        match rename.clone() {
                            Some(target) => this.begin_rename(target, RenameAnchor::AgentRow),
                            None => {
                                this.terminal.live.status =
                                    crate::shell::rename::NOT_CONNECTED.into();
                            }
                        }
                        cx.notify();
                    })),
            )
            .children(unavailable.map(|why| {
                div()
                    .px(px(metric::SIDE))
                    .pb(px(metric::MENU_PADDING))
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::DISABLED))
                    .child(why)
            }));
        Some(
            deferred(anchored().position(menu.at).snap_to_window().child(list))
                .with_priority(1)
                .into_any_element(),
        )
    }
}

pub(super) fn long_press_layer(row: PressedRow, cx: &mut Context<Smoke>) -> impl IntoElement {
    row_press_layer(cx, move |app, at| app.sidebar_long_press(row.clone(), at))
}

pub(super) fn agent_press_layer(menu: AgentMenu, cx: &mut Context<Smoke>) -> impl IntoElement {
    row_press_layer(cx, move |app, at| {
        app.shell.sidebar.long_press = None;
        app.shell.sidebar.agent_menu = Some(AgentMenu { at, ..menu.clone() });
    })
}

/// Handles a native long press without forwarding it to the terminal.
pub(crate) fn row_press_layer(
    cx: &mut Context<Smoke>,
    open: impl Fn(&mut Smoke, Point<Pixels>) + Clone + 'static,
) -> impl IntoElement {
    let entity = cx.entity().downgrade();
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            let open = open.clone();
            let entity = entity.clone();
            window.on_mouse_event(move |event: &LongPressEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.phase != TouchPhase::Started
                    || window.default_prevented()
                    || !hitbox.is_hovered_at(event.start_position, window)
                {
                    return;
                }
                let _ = entity.update(cx, |this, cx| {
                    open(this, event.start_position);
                    cx.notify();
                });
                window.prevent_default();
                cx.stop_propagation();
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
