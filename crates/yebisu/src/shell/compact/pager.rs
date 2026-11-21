use crate::{
    app::Smoke,
    board::{Navigation, Target, snapshot},
    core::Command,
    layout::compact_window,
    shell::{
        fit::{docked_keyboard_height, ime_keyboard},
        style::{color, metric, text},
    },
};
use gpui::{
    AnyElement, Context, DispatchPhase, HitboxBehavior, MouseButton, Role, TouchDragEvent,
    TouchPhase, WeakEntity, Window, canvas, div, prelude::*, px, rgb,
};
use sheprd_client::protocol::ClientShellSnapshot;

// The docked keyboard was 498 pt on the 13-inch Simulator. The 71 pt
// assistant strip alone must leave the split in place (canvas.json:226).
const SOFTWARE_KEYBOARD_MIN_HEIGHT: f32 = 120.0;
// PanePager.dc.html:122 and Compact.dc.html:35.
const PAGER_LABEL_GAP: f32 = 4.0;
const MAX_DOTS: usize = 3;
// bf7a14860, ipad_keys/band.rs:271 (points).
const SWIPE_MIN: f32 = 32.0;
const TAP_SLOP: f32 = 8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    At(usize),
    Next,
    Previous,
}

impl Page {
    fn index(self, current: usize, count: usize) -> Option<usize> {
        if current >= count {
            return None;
        }
        let index = match self {
            Self::At(index) => index,
            Self::Next => (current + 1) % count,
            Self::Previous => current.checked_sub(1).unwrap_or(count - 1),
        };
        (index < count && index != current).then_some(index)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Panes {
    tab: String,
    ids: Vec<String>,
    focused: usize,
}

impl Panes {
    fn from_snapshot(snapshot: &ClientShellSnapshot) -> Option<Self> {
        let tab = snapshot.focused_tab_id.as_ref()?;
        let workspace = snapshot.focused_workspace_id.as_ref()?;
        let panes: Vec<_> = snapshot
            .panes
            .iter()
            .filter(|pane| &pane.tab_id == tab && &pane.workspace_id == workspace)
            .collect();
        let focused = panes.iter().position(|pane| {
            snapshot
                .focused_pane_id
                .as_ref()
                .map_or(pane.focused, |id| *id == pane.pane_id)
        })?;
        let ids = panes.into_iter().map(|pane| pane.pane_id.clone()).collect();
        Some(Self {
            tab: tab.clone(),
            ids,
            focused,
        })
    }

    fn label(&self, index: usize) -> String {
        format!("Pane {} of {}", index + 1, self.ids.len())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Pager {
    host: String,
    generation: u64,
    boot: String,
    core: bool,
    panes: Panes,
}

impl Smoke {
    pub(in crate::shell) fn pager_available_at(&self, window: &Window) -> bool {
        let viewport = window.viewport_size();
        let size = crate::layout::Size::new(viewport.width.into(), viewport.height.into());
        compact_window(size.width)
            || docked_keyboard_height(size, ime_keyboard(size, &self.platform_insets(window).ime))
                > SOFTWARE_KEYBOARD_MIN_HEIGHT
    }

    pub(in crate::shell) fn focused_pane_count(&self) -> usize {
        let Some((snapshot, _)) = self.terminal.coherent_frame() else {
            return 0;
        };
        Panes::from_snapshot(&snapshot).map_or(0, |panes| panes.ids.len())
    }

    fn pager(&self) -> Option<Pager> {
        if !self.terminal.live.connected || self.input_modal() {
            return None;
        }
        let (snapshot, surface) = self.terminal.coherent_frame()?;
        if surface.popup.is_some() {
            return None;
        }
        let mut panes = Panes::from_snapshot(&snapshot)?;
        if panes.ids.len() < 2 {
            return None;
        }
        let core = self.terminal.core_mode;
        if core {
            // The wire-shaped client snapshot calls every Core tab "core".
            // Use the worker's actual tab ID to fence a delayed touch.
            let tab = self.core_tabs().into_iter().find(|tab| tab.active)?;
            if tab.panes != panes.ids || tab.focused_pane != panes.focused {
                return None;
            }
            panes.tab = tab.id;
        }
        Some(Pager {
            host: self.terminal.reconnect.target.clone(),
            generation: self.terminal.link.generation,
            boot: snapshot.boot_id.clone(),
            core,
            panes,
        })
    }

    pub(in crate::shell) fn render_pane_pager(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pager = self.pager()?;
        let focused = pager.panes.focused;
        let count = pager.panes.ids.len();
        let owner = cx.weak_entity();
        let mut row = drag_target(
            div()
                .id("shell-pane-pager")
                .debug_selector(|| "shell-pane-pager".into())
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .min_w_0()
                .h(px(metric::TOUCH))
                .flex_none(),
            owner.clone(),
            pager.clone(),
            None,
        )
        .child(
            div()
                .mr(px(PAGER_LABEL_GAP))
                .whitespace_nowrap()
                .text_size(px(text::SECONDARY))
                .child(pager.panes.label(focused)),
        );
        let first = focused
            .saturating_sub(1)
            .min(count.saturating_sub(MAX_DOTS));
        for index in first..count.min(first + MAX_DOTS) {
            let shown = pager.clone();
            let dot = div()
                .id(("shell-pane-dot", index))
                .debug_selector(move || format!("shell-pane-dot-{index}"))
                .role(Role::Button)
                .aria_label(pager.panes.label(index))
                .aria_selected(index == focused)
                .size(px(metric::TOUCH))
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(metric::SMALL_DOT))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(color::TEXT))
                        .when(index == focused, |dot| dot.bg(rgb(color::TEXT))),
                )
                .children(match index {
                    0 => crate::qa_ax::mark("qa.pager.pane.1", "Pane 1"),
                    1 => crate::qa_ax::mark("qa.pager.pane.2", "Pane 2"),
                    _ => None,
                })
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.turn_pane_page(&shown, Page::At(index), window, cx);
                }));
            row = row.child(drag_target(
                dot,
                owner.clone(),
                pager.clone(),
                Some(Page::At(index)),
            ));
        }
        Some(row.into_any_element())
    }

    fn turn_pane_page(
        &mut self,
        shown: &Pager,
        page: Page,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pager_available_at(window) || self.pager().as_ref() != Some(shown) {
            return;
        }
        let Some(index) = page.index(shown.panes.focused, shown.panes.ids.len()) else {
            return;
        };
        let result = if shown.core {
            self.try_core_command(Command::FocusAddress(shown.panes.ids[index].clone()))
        } else {
            self.input_transition(|this| this.focus_herdr1_page(shown, &shown.panes.ids[index]))
        };
        if let Err(error) = result {
            self.terminal.live.status = error.to_string();
        }
        cx.notify();
    }

    fn focus_herdr1_page(&mut self, shown: &Pager, id: &str) -> crate::Result<()> {
        let unavailable = || crate::Error::Ssh("Pane unavailable; try again".into());
        let terminal = &mut self.terminal;
        let mut live = terminal.link.inbox.try_lock().map_err(|_| unavailable())?;
        let current = snapshot(&live).ok_or_else(unavailable)?;
        if !live.connected
            || live.version_mismatch.get().is_some()
            || live
                .surface
                .as_ref()
                .is_some_and(|surface| surface.popup.is_some())
            || terminal.navigating.is_some()
            || current.boot_id != shown.boot
            || Panes::from_snapshot(current).as_ref() != Some(&shown.panes)
        {
            return Err(unavailable());
        }
        let handle = terminal.link.handle.as_ref().ok_or_else(unavailable)?;
        let request = handle
            .focus_pane(&shown.boot, id)
            .map_err(|error| crate::Error::Ssh(error.to_string()))?;
        terminal.navigating = Some(Navigation {
            boot: shown.boot.clone(),
            revision: current.revision,
            target: Target::Agent(id.to_owned()),
            request,
        });
        terminal.input.detach();
        live.begin_navigation(terminal.navigating.clone());
        Ok(())
    }
}

// A dot registers after the strip so its bubble listener gets first refusal.
fn drag_target<E: ParentElement>(
    element: E,
    owner: WeakEntity<Smoke>,
    pager: Pager,
    tap: Option<Page>,
) -> E {
    element.child(
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || !hitbox.is_hovered_at(event.start_position, window)
                        || window.default_prevented()
                    {
                        return;
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    if let Some(page) = drag_page(event, tap) {
                        let _ = owner.update(cx, |this, cx| {
                            this.turn_pane_page(&pager, page, window, cx);
                        });
                    }
                });
            },
        )
        .absolute()
        .inset_0(),
    )
}

fn drag_page(event: &TouchDragEvent, tap: Option<Page>) -> Option<Page> {
    if event.phase != TouchPhase::Ended {
        return None;
    }
    let delta = event.position - event.start_position;
    let x = f32::from(delta.x);
    let y = f32::from(delta.y);
    if x.abs() >= SWIPE_MIN && x.abs() > y.abs() {
        Some(if x < 0.0 { Page::Next } else { Page::Previous })
    } else if x.abs() <= TAP_SLOP && y.abs() <= TAP_SLOP {
        tap
    } else {
        None
    }
}

#[cfg(test)]
#[path = "pager_tests.rs"]
mod pager_tests;
