//! Scroll motion over the terminal (a finger's pan and its momentum, a
//! trackpad's two fingers, a wheel) goes to the pane under the pointer as
//! Herdr wheel input: Herdr scrolls its scrollback, or reports the wheel to a
//! program that reads the mouse, or sends the alternate screen's arrow keys.
use crate::{
    app::Smoke,
    input_geometry::InputGeometry,
    terminal::{InputTarget, popup_origin},
    terminal_input::Destination,
    upstream::wheel::{DEFAULT_MOUSE_SCROLL_LINES, WheelTarget, wheel_target},
};
use gpui::{Context, DispatchPhase, HitboxBehavior, ScrollEvent, Window, canvas, prelude::*};
use sheprd_client::{
    Method,
    protocol::{ClientMouseKind, ClientMousePosition, ClientPaneInputEvent, PaneSurfaceFrame},
};

impl Smoke {
    /// A hitbox over the terminal's content area that takes scroll events.
    /// It paints nothing. Absolute, so it lies over the edit menu's canvas
    /// instead of being laid out below it and clipped.
    pub(crate) fn terminal_wheel_canvas(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity().downgrade();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
                    // An occluding sheet or popover in front takes the scroll.
                    if phase != DispatchPhase::Bubble || !hitbox.should_handle_scroll(window) {
                        return;
                    }
                    let _ = entity.update(cx, |this, cx| this.terminal_wheel(event, window, cx));
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    pub(crate) fn terminal_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.front_layers.covers(event.position, window)
            || self.find.is_some()
            || self.find_handoff.paused
            || self.terminal.input.composition.marked.is_some()
            || self.input_modal()
            || self.terminal.navigating.is_some()
            || self.actions.palette
            || self.shell.shortcuts.open
        {
            return;
        }
        self.refresh_terminal_input();
        self.sync_handoff();
        if self.handoff.blocks_input()
            || !self.terminal.input.ready
            || self.terminal.input.holds_size()
        {
            return;
        }
        let Some(destination) = self.terminal.input.destination.clone() else {
            return;
        };
        let Some((_, surface)) = self.terminal.coherent_frame() else {
            return;
        };
        let geometry = self.terminal.geometry;
        let Some(target) = pointed_target(&geometry, &surface, event.position) else {
            return;
        };
        cx.stop_propagation();
        let steps = self.terminal.wheel.steps(
            &target,
            event,
            geometry.cell_width,
            geometry.cell_height,
            DEFAULT_MOUSE_SCROLL_LINES,
        );
        let events = target
            .wheel_events(steps, event.modifiers)
            .collect::<Vec<_>>();
        if events.is_empty() {
            return;
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let receipt = wheel_receipt(&target.target, &events);
        if let Err(error) = self.send_wheel(&destination, &target.target, events) {
            self.terminal.live.status = error.to_string();
            self.terminal.wheel = Default::default();
            cx.notify();
            return;
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        self.observation.record(receipt);
        cx.notify();
    }

    /// Herdr 1 takes the pane's events in one message; the Herdr 2 worker
    /// writes each to the pane's own terminal.
    fn send_wheel(
        &self,
        destination: &Destination,
        pane: &InputTarget,
        events: Vec<ClientPaneInputEvent>,
    ) -> crate::Result<()> {
        let Some(core) = &self.terminal.core else {
            return self.terminal.link.send_wheel(destination, pane, events);
        };
        let InputTarget::Pane(id) = pane else {
            return Ok(());
        };
        for event in events {
            core.send(crate::core::Command::Wheel {
                destination: destination.clone(),
                pane: id.clone(),
                event,
            })?;
        }
        Ok(())
    }

    /// Starting a composition while the focused pane shows its scrollback
    /// returns it to the bottom first, so the composition and its candidate
    /// window sit at the input position (owner decision 6).
    pub(crate) fn compose_started_at_bottom(&mut self, window: &mut Window) {
        let Some(InputTarget::Pane(id)) = self
            .terminal
            .input
            .destination
            .as_ref()
            .map(|destination| destination.target.clone())
        else {
            return;
        };
        if self.scroll_to_bottom(&id) {
            // UIKit asks for the candidate rectangle again once the bottom lands.
            window.invalidate_character_coordinates();
        }
    }

    /// Returns `pane_id` to its bottom when it shows its scrollback. Herdr 1
    /// gets `pane.scroll`, or the wheel when it lacks that method; a Herdr 2
    /// pane's emulator gets the wheel. The wheel only ever goes to a pane that
    /// reads neither the mouse nor the alternate screen: a program would read
    /// it as reports or arrow keys. Returns whether a request was made.
    pub(crate) fn scroll_to_bottom(&mut self, pane_id: &str) -> bool {
        let Some(destination) = self.terminal.input.destination.clone() else {
            return false;
        };
        let Some(surface) = self.terminal.live.surface.clone() else {
            return false;
        };
        let Some(pane) = surface.panes.iter().find(|pane| pane.pane_id == pane_id) else {
            return false;
        };
        let Some(offset) = pane
            .scroll
            .map(|scroll| scroll.offset_from_bottom)
            .filter(|offset| *offset > 0)
        else {
            return false;
        };
        let wheel = |offset: u64| ClientPaneInputEvent::Mouse {
            kind: ClientMouseKind::ScrollDown,
            position: ClientMousePosition::Cell { column: 0, row: 0 },
            geometry: None,
            modifiers: 0,
            lines: u16::try_from(offset).unwrap_or(u16::MAX),
        };
        let reads_wheel = pane.mouse_reporting || pane.alternate_screen_active;
        let result = if let Some(core) = &self.terminal.core {
            if reads_wheel {
                return false;
            }
            // The emulator scrolls its own history; one event, any count.
            core.send(crate::core::Command::Wheel {
                destination: destination.clone(),
                pane: pane_id.to_owned(),
                event: wheel(offset),
            })
        } else if Method::PaneScroll.advertised_in(&self.terminal.live.methods) {
            let Some(handle) = &self.terminal.link.handle else {
                return false;
            };
            handle
                .request(
                    &destination.boot,
                    Method::PaneScroll,
                    serde_json::json!({"pane_id": pane_id, "offset_from_bottom": 0}),
                )
                .map(drop)
                .map_err(|error| crate::Error::Ssh(error.to_string()))
        } else if !reads_wheel {
            // Without the method, the wheel scrolls the scrollback back down.
            self.terminal.link.send_wheel(
                &destination,
                &InputTarget::Pane(pane_id.to_owned()),
                vec![wheel(offset)],
            )
        } else {
            return false;
        };
        if let Err(error) = result {
            self.terminal.live.status = error.to_string();
        }
        true
    }
}

/// The pane or popup under `position` as the terminal draws it. Panes are
/// drawn framed, not at their cell rectangle, so the drawn cell is mapped
/// back to the surface's own coordinates first.
pub(crate) fn pointed_target(
    geometry: &InputGeometry,
    surface: &PaneSurfaceFrame,
    position: gpui::Point<gpui::Pixels>,
) -> Option<WheelTarget> {
    let (column, row) = geometry.cell_at(surface, position, None)?;
    let (cell_width, cell_height) = (geometry.cell_width, geometry.cell_height);
    let (x, y) = surface.popup.as_ref().map_or((0., 0.), |popup| {
        let origin = popup_origin(&surface.frame, &popup.frame, cell_width, cell_height);
        (f32::from(origin.x), f32::from(origin.y))
    });
    wheel_target(
        surface,
        x + (f32::from(column) + 0.5) * cell_width,
        y + (f32::from(row) + 0.5) * cell_height,
        cell_width,
        cell_height,
    )
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn wheel_receipt(target: &InputTarget, events: &[ClientPaneInputEvent]) -> serde_json::Value {
    let (InputTarget::Pane(id) | InputTarget::Popup(id)) = target;
    let events = events
        .iter()
        .filter_map(|event| match event {
            ClientPaneInputEvent::Mouse { kind, lines, .. } => {
                Some(serde_json::json!({"kind": format!("{kind:?}"), "lines": lines}))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    serde_json::json!({"kind": "terminal-wheel", "pane": id, "events": events})
}

#[cfg(test)]
#[path = "terminal_wheel_tests.rs"]
mod tests;
