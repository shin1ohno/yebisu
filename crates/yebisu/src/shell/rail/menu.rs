use super::{ITEM_GAP, RailItem, RailItemKind, RailTarget, rail_items};
use crate::{
    app::Smoke,
    board::{Target, snapshot},
    layout::RAIL_WIDTH,
    shell::{
        HerdrKind,
        host_menus::{HostMenuItem, MenuItemId, MenuScope},
        style::{color, metric, radius, shadow, weight},
    },
};
use gpui::{
    AnyElement, Context, DispatchPhase, HitboxBehavior, LongPressEvent, Pixels, Point, Role,
    TouchPhase, Window, anchored, canvas, deferred, div, point, prelude::*, px, rgb,
};
use sheprd_commands::Command;
use std::time::Duration;

const MENU_WIDTH: f32 = 240.0; // canvas.json, note "railmenu".
const COUNT_SIZE: f32 = 10.0; // canvas.json, note "railmenu".

#[derive(Clone, Debug)]
pub(super) struct RailMenu {
    item: RailItem,
    at: Point<Pixels>,
    tab_target: Option<RailTarget>,
    needs: usize,
}

#[derive(Clone, Copy)]
enum Action {
    Open,
    NewTab,
    Detach,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Open => "rail-menu-open",
            Self::NewTab => "rail-menu-new-tab",
            Self::Detach => "rail-menu-detach",
        }
    }

    fn label(self, item: &RailItem) -> String {
        match self {
            Self::Open => "Open".into(),
            Self::NewTab => "New tab".into(),
            Self::Detach => format!("Detach {}", item.name),
        }
    }
}

impl Smoke {
    pub(super) fn open_rail_target(&mut self, target: &RailTarget, window: &mut Window) {
        window.dismiss_virtual_keyboard();
        self.dismiss_rail_menu();
        match target {
            RailTarget::Host(host) => self.select_host(host),
            RailTarget::Space {
                host,
                workspace_id,
                boot,
                generation,
            } => {
                if let Err(error) = self.navigate_board(
                    host,
                    Target::Space(workspace_id.clone()),
                    boot,
                    *generation,
                ) {
                    self.terminal.live.status = error.to_string();
                }
            }
        }
    }

    pub(super) fn dismiss_rail_menu(&mut self) {
        self.shell.rail.menu = None;
        self.shell.rail.menu_generation = self.shell.rail.menu_generation.wrapping_add(1);
    }

    pub(super) fn render_rail_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.shell.rail.menu.clone()?;
        if self.input_modal() {
            self.dismiss_rail_menu();
            return None;
        }

        let actions = [Action::Open, Action::NewTab]
            .into_iter()
            .chain((menu.item.kind == RailItemKind::Host).then_some(Action::Detach));
        let panel = div()
            .id("rail-row-menu")
            .debug_selector(|| "rail-row-menu".to_owned())
            .role(Role::Menu)
            .w(px(MENU_WIDTH))
            .rounded(px(radius::POPOVER))
            .py(px(metric::MENU_PADDING))
            .bg(rgb(color::RAISED))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.dismiss_rail_menu();
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(metric::INLINE_GAP))
                    .h(px(metric::TOUCH))
                    .px_3()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(weight::SEMIBOLD)
                            .child(menu.item.name.clone()),
                    )
                    .when(menu.needs > 0, |header| {
                        header.child(
                            div()
                                .flex_none()
                                .text_size(px(COUNT_SIZE))
                                .text_color(rgb(color::YELLOW))
                                .child(format!("{} needs you", menu.needs)),
                        )
                    }),
            )
            .children(actions.map(|action| {
                let reason = self.rail_action_unavailable(action, &menu);
                div()
                    .id(action.name())
                    .debug_selector(move || action.name().to_owned())
                    .role(Role::MenuItem)
                    .h(px(metric::TOUCH))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .text_color(rgb(if reason.is_some() {
                        color::DISABLED
                    } else {
                        color::TEXT
                    }))
                    .child(div().truncate().child(action.label(&menu.item)))
                    .when_some(reason, |row, reason| {
                        row.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                            .child(
                                div()
                                    .text_size(px(crate::shell::style::text::SMALL))
                                    .child(reason),
                            )
                    })
                    .when(reason.is_none(), |row| {
                        row.on_click(cx.listener(move |this, _, window, cx| {
                            if let Err(error) = this.pick_rail_action(action, window, cx) {
                                this.terminal.live.status = error.to_string();
                            }
                            cx.notify();
                        }))
                    })
            }));
        Some(
            deferred(anchored().position(menu.at).snap_to_window().child(panel))
                .with_priority(1)
                .into_any_element(),
        )
    }

    fn rail_action_unavailable(&self, action: Action, menu: &RailMenu) -> Option<&'static str> {
        let item = &menu.item;
        match action {
            Action::Open => None,
            Action::NewTab if item.herdr == HerdrKind::Herdr2 => {
                Some("New tab is not available on Herdr 2")
            }
            Action::NewTab if !item.connected => Some("Not connected"),
            Action::NewTab if menu.tab_target.is_none() => Some("No space selected"),
            Action::NewTab => None,
            Action::Detach => {
                let RailTarget::Host(host) = &item.target else {
                    return Some("Select a host");
                };
                let state = self.menu_chrome_state();
                crate::shell::host_menus::host_long_press_for(
                    &state,
                    host,
                    &self.current_menu_scope(),
                )
                .into_iter()
                .find(|entry| entry.item == MenuItemId::Host(HostMenuItem::Detach))
                .and_then(|entry| entry.reason)
            }
        }
    }

    fn new_rail_menu(&self, item: &RailItem, at: Point<Pixels>) -> RailMenu {
        let needs = match &item.target {
            RailTarget::Host(host) => self
                .rail_hosts()
                .into_iter()
                .find(|row| &row.name == host)
                .map_or(0, |row| {
                    row.spaces
                        .into_iter()
                        .flat_map(|space| space.agents)
                        .filter(|status| {
                            *status == crate::shell::sidebar_status::SidebarStatus::NeedsYou
                        })
                        .count()
                }),
            RailTarget::Space { .. } => item.needs,
        };
        RailMenu {
            item: item.clone(),
            at,
            tab_target: self.rail_tab_target(item),
            needs,
        }
    }

    fn rail_tab_target(&self, item: &RailItem) -> Option<RailTarget> {
        let RailTarget::Host(host) = &item.target else {
            return Some(item.target.clone());
        };
        let current = host == &self.terminal.reconnect.target;
        let terminal = if current {
            &self.terminal
        } else {
            self.board.hidden.get(host)?
        };
        let workspace_id = snapshot(&terminal.live)?.focused_workspace_id.clone()?;
        Some(RailTarget::Space {
            host: host.clone(),
            workspace_id,
            boot: item.boot.clone(),
            generation: item.generation,
        })
    }

    fn pick_rail_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let menu = self
            .shell
            .rail
            .menu
            .take()
            .ok_or(crate::Error::NativeMenu("no rail row menu"))?;
        let item = &menu.item;
        if self.input_modal()
            || !rail_items(&self.rail_hosts()).iter().any(|row| {
                row.target == item.target
                    && row.boot == item.boot
                    && row.generation == item.generation
                    && row.herdr == item.herdr
            })
        {
            return Err(crate::Error::NativeMenu("menu target changed"));
        }
        if let Some(reason) = self.rail_action_unavailable(action, &menu) {
            return Err(crate::Error::NativeMenu(reason));
        }
        match action {
            Action::Open => self.open_rail_target(&item.target, window),
            Action::NewTab => {
                let Some(RailTarget::Space {
                    host,
                    workspace_id,
                    boot,
                    generation,
                }) = menu.tab_target
                else {
                    return Err(crate::Error::NativeMenu("no space was shown"));
                };
                self.new_rail_tab(host, workspace_id, boot, generation, window, cx)?;
            }
            Action::Detach => {
                let RailTarget::Host(host) = &item.target else {
                    return Err(crate::Error::NativeMenu("no host was shown"));
                };
                self.run_menu_item(
                    MenuItemId::Host(HostMenuItem::Detach),
                    &MenuScope {
                        host: host.clone(),
                        space: None,
                    },
                    window,
                    cx,
                )?;
            }
        }
        Ok(())
    }

    fn new_rail_tab(
        &mut self,
        host: String,
        workspace: String,
        boot: String,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let focused = |this: &Self, workspace: &str| {
            snapshot(&this.terminal.live)
                .is_some_and(|shown| shown.focused_workspace_id.as_deref() == Some(workspace))
        };
        if self.terminal.reconnect.target == host && focused(self, &workspace) {
            return self.run_action(Command::Tab);
        }
        self.navigate_board(&host, Target::Space(workspace.clone()), &boot, generation)?;
        let started = self
            .terminal
            .navigating
            .as_ref()
            .ok_or(crate::Error::NativeMenu("no rail navigation"))?
            .request
            .clone();
        let serial = self.shell.rail.menu_generation;
        let timer = cx.background_executor().clone();
        cx.spawn_in(window, async move |entity, cx| {
            // Wait for this navigation's projected focus before creating in it.
            // Another rail action, target or connection supersedes the request.
            for _ in 0..100 {
                let waiting = entity.update_in(cx, |this, _, cx| {
                    let terminal = &this.terminal;
                    if this.shell.rail.menu_generation != serial
                        || terminal.reconnect.target != host
                        || terminal.link.generation != generation
                        || !terminal.live.connected
                        || !snapshot(&terminal.live).is_some_and(|shown| shown.boot_id == boot)
                        || this.input_modal()
                    {
                        return false;
                    }
                    if let Some(navigation) = &terminal.navigating {
                        return navigation.request == started;
                    }
                    if terminal.live.failed_request.as_ref() != Some(&started)
                        && focused(this, &workspace)
                    {
                        if let Err(error) = this.run_action(Command::Tab) {
                            this.terminal.live.status = error.to_string();
                        }
                        cx.notify();
                    }
                    false
                });
                if !matches!(waiting, Ok(true)) {
                    return;
                }
                timer.timer(Duration::from_millis(50)).await;
            }
        })
        .detach();
        Ok(())
    }
}

pub(super) fn long_press_layer(item: RailItem, cx: &mut Context<Smoke>) -> impl IntoElement {
    let entity = cx.entity().downgrade();
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            let item = item.clone();
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
                    if !this.input_modal() {
                        this.dismiss_rail_menu();
                        let at = point(px(RAIL_WIDTH + ITEM_GAP), event.start_position.y);
                        this.shell.rail.menu = Some(this.new_rail_menu(&item, at));
                        cx.notify();
                    }
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
