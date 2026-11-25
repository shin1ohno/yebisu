// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/terminal.rs at 398c132a06723a0cc1494842a34666d173ec0200
use crate::upstream::terminal::{InputTarget, popup_origin};
use gpui::{Bounds, Modifiers, Pixels, ScrollDelta, ScrollEvent, TouchPhase, point, px, size};
use sheprd_client::protocol::{
    ClientMouseGeometry, ClientMouseKind, ClientMousePosition, ClientPaneInputEvent,
    PaneSurfaceFrame, SurfaceRect,
};
use std::num::NonZeroU16;

/// Lines GPUI already reports for one discrete wheel notch. Linux and FreeBSD
/// use GPUI's fixed three; Windows applies the system wheel setting, assumed
/// at its default of three; macOS reports about one, with acceleration.
#[cfg(target_os = "macos")]
pub(crate) const PLATFORM_NOTCH_LINES: u16 = 1;
// herdr-uikit sends one ScrollDelta::Lines(+-1) per wheel notch.
#[cfg(target_os = "ios")]
pub(crate) const PLATFORM_NOTCH_LINES: u16 = 1;
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub(crate) const PLATFORM_NOTCH_LINES: u16 = 3;

/// Herdr's default `ui.mouse_scroll_lines`. yebisu does not read Herdr's
/// configuration, so a notch scrolls this many lines.
pub(crate) const DEFAULT_MOUSE_SCROLL_LINES: NonZeroU16 = match NonZeroU16::new(3) {
    Some(lines) => lines,
    None => NonZeroU16::MIN,
};

/// Whole wheel steps on each axis, signed as GPUI reports them: positive
/// lines scroll up and positive columns scroll left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WheelSteps {
    pub lines: i16,
    pub columns: i16,
}

/// Sub-step motion kept between events on one axis.
#[derive(Default)]
struct WheelRemainder(f32);

impl WheelRemainder {
    fn add(&mut self, delta: f32) -> i16 {
        if !delta.is_finite() {
            return 0;
        }
        if delta != 0. && delta.signum() != self.0.signum() {
            self.0 = 0.;
        }
        // Bound each event's work; keep sub-cell trackpad motion, not an input backlog.
        let total = (self.0 + delta).clamp(-128., 128.);
        let steps = total.trunc() as i16;
        self.0 = total - f32::from(steps);
        steps
    }
}

/// Turns wheel and trackpad motion into whole lines and columns for one
/// target, resetting when the target or the gesture changes.
///
/// GPUI's platforms already turn Shift with a vertical wheel into horizontal
/// motion (macOS in the OS, X11 and Wayland in GPUI), so the axes are taken as
/// reported and Shift is forwarded unchanged rather than swapped again.
#[derive(Default)]
pub struct WheelAccumulator {
    target: Option<InputTarget>,
    lines: WheelRemainder,
    columns: WheelRemainder,
}

impl WheelAccumulator {
    pub fn steps(
        &mut self,
        target: &WheelTarget,
        event: &ScrollEvent,
        cell_width: f32,
        cell_height: f32,
        notch_lines: NonZeroU16,
    ) -> WheelSteps {
        if self.target.as_ref() != Some(&target.target)
            || matches!(event.touch_phase, TouchPhase::Started)
        {
            *self = Self {
                target: Some(target.target.clone()),
                ..Self::default()
            };
        }
        // Dominance compares the reported motion itself: cells are not square.
        let (raw_x, raw_y, x) = match event.delta {
            ScrollDelta::Pixels(delta) => {
                let x = delta.x.to_f64() as f32;
                (x, delta.y.to_f64() as f32, x / cell_width)
            }
            ScrollDelta::Lines(delta) => (delta.x, delta.y, delta.x),
        };
        let lines = self.lines.add(wheel_rows(event, cell_height, notch_lines));
        // Herdr turns a horizontal wheel only into a mouse report, so motion
        // over a pane that does not report the mouse would do nothing. A
        // mostly vertical swipe's sideways drift is not a column either.
        let columns = if !target.mouse_reporting {
            self.columns = WheelRemainder::default();
            0
        } else if raw_x.abs() > raw_y.abs() {
            self.columns.add(x)
        } else {
            0
        };
        WheelSteps { lines, columns }
    }

    /// Forgets the vertical motion kept between events, once another path
    /// has consumed it.
    pub(crate) fn drop_lines(&mut self) {
        self.lines = WheelRemainder::default();
    }
}

/// The rows `event` scrolls, up into history being positive. A discrete wheel
/// notch scrolls `notch_lines`, Herdr's `ui.mouse_scroll_lines`, on both the
/// smooth and whole-line paths; precise (trackpad) pixel motion keeps
/// following the pointer one cell height per row.
pub(crate) fn wheel_rows(event: &ScrollEvent, cell_height: f32, notch_lines: NonZeroU16) -> f32 {
    match event.delta {
        ScrollDelta::Pixels(delta) => delta.y.to_f64() as f32 / cell_height,
        // Multiply before dividing so the common equal counts stay exact.
        ScrollDelta::Lines(delta) => {
            delta.y * f32::from(notch_lines.get()) / f32::from(PLATFORM_NOTCH_LINES)
        }
    }
}

#[derive(PartialEq)]
pub struct WheelTarget {
    pub target: InputTarget,
    pub(crate) mouse_reporting: bool,
    /// Whether the wheel goes to an application, as mouse reports or as the
    /// alternate screen's arrow keys, rather than to the scrollback. Herdr
    /// sends such an application one report per event whatever its count, so
    /// a count must travel as that many events.
    stepwise: bool,
    pub(crate) bounds: Bounds<Pixels>,
    position: ClientMousePosition,
    geometry: Option<ClientMouseGeometry>,
}

impl WheelTarget {
    /// The events for `steps`, vertical first. Scrollback gets one event per
    /// moving axis carrying its whole count, which Herdr scrolls by. An
    /// application reading the wheel gets one single-step event per step,
    /// since Herdr turns each event into exactly one report or key for it, as
    /// a terminal sends one report per wheel step.
    pub fn wheel_events(
        &self,
        steps: WheelSteps,
        modifiers: Modifiers,
    ) -> impl Iterator<Item = ClientPaneInputEvent> + '_ {
        let vertical = (steps.lines != 0).then(|| {
            let kind = if steps.lines > 0 {
                ClientMouseKind::ScrollUp
            } else {
                ClientMouseKind::ScrollDown
            };
            (kind, steps.lines)
        });
        let horizontal = (steps.columns != 0).then(|| {
            let kind = if steps.columns > 0 {
                ClientMouseKind::ScrollLeft
            } else {
                ClientMouseKind::ScrollRight
            };
            (kind, steps.columns)
        });
        vertical
            .into_iter()
            .chain(horizontal)
            .flat_map(move |(kind, count)| {
                let count = count.unsigned_abs();
                let (events, each) = if self.stepwise {
                    (count, 1)
                } else {
                    (1, count)
                };
                (0..events).map(move |_| {
                    let mut event = self.mouse_event(kind, modifiers);
                    if let ClientPaneInputEvent::Mouse { lines, .. } = &mut event {
                        *lines = each;
                    }
                    event
                })
            })
    }

    /// The cell under the pointer, also for an application that asked for pixels.
    pub(crate) fn cell(&self) -> (u16, u16) {
        match self.position {
            ClientMousePosition::Cell { column, row }
            | ClientMousePosition::Pixels { column, row, .. } => (column, row),
        }
    }

    pub(crate) fn mouse_event(
        &self,
        kind: ClientMouseKind,
        modifiers: Modifiers,
    ) -> ClientPaneInputEvent {
        ClientPaneInputEvent::Mouse {
            kind,
            position: self.position,
            geometry: self.geometry,
            modifiers: u8::from(modifiers.shift)
                | (u8::from(modifiers.control) << 1)
                | (u8::from(modifiers.alt) << 2)
                | (u8::from(modifiers.platform) << 3),
            lines: 1,
        }
    }
}

pub fn wheel_target(
    surface: &PaneSurfaceFrame,
    x: f32,
    y: f32,
    cell_width: f32,
    cell_height: f32,
) -> Option<WheelTarget> {
    if !x.is_finite()
        || !y.is_finite()
        || !cell_width.is_finite()
        || !cell_height.is_finite()
        || cell_width <= 0.
        || cell_height <= 0.
        || x < 0.
        || y < 0.
    {
        return None;
    }
    let (target, rect, mouse_reporting, pixel_mouse, width_px, height_px, origin_x, origin_y) =
        if let Some(popup) = &surface.popup {
            let origin = popup_origin(&surface.frame, &popup.frame, cell_width, cell_height);
            (
                InputTarget::Popup(popup.terminal_id.clone()),
                SurfaceRect {
                    x: 0,
                    y: 0,
                    width: popup.frame.width,
                    height: popup.frame.height,
                },
                popup.mouse_reporting,
                popup.sgr_pixel_mouse,
                popup.pixel_width,
                popup.pixel_height,
                origin.x.to_f64() as f32,
                origin.y.to_f64() as f32,
            )
        } else {
            let pane = surface.panes.iter().find(|pane| {
                let r = pane.inner_rect;
                x >= r.x as f32 * cell_width
                    && x < (u32::from(r.x) + u32::from(r.width)) as f32 * cell_width
                    && y >= r.y as f32 * cell_height
                    && y < (u32::from(r.y) + u32::from(r.height)) as f32 * cell_height
            })?;
            (
                InputTarget::Pane(pane.pane_id.clone()),
                pane.inner_rect,
                pane.mouse_reporting,
                pane.sgr_pixel_mouse,
                pane.pixel_width,
                pane.pixel_height,
                pane.inner_rect.x as f32 * cell_width,
                pane.inner_rect.y as f32 * cell_height,
            )
        };
    let x = x - origin_x;
    let y = y - origin_y;
    if x < 0.
        || y < 0.
        || x >= rect.width as f32 * cell_width
        || y >= rect.height as f32 * cell_height
    {
        return None;
    }
    let column = (x / cell_width).floor() as u16;
    let row = (y / cell_height).floor() as u16;
    let geometry = (pixel_mouse && width_px > 0 && height_px > 0).then_some(ClientMouseGeometry {
        cols: rect.width,
        rows: rect.height,
        width_px,
        height_px,
    });
    let position = if geometry.is_some() {
        ClientMousePosition::Pixels {
            x: (x / (rect.width as f32 * cell_width) * width_px as f32).floor() as u32,
            y: (y / (rect.height as f32 * cell_height) * height_px as f32).floor() as u32,
            column,
            row,
        }
    } else {
        ClientMousePosition::Cell { column, row }
    };
    // A popup does not say whether it is on the alternate screen.
    let stepwise = mouse_reporting
        || matches!(&target, InputTarget::Pane(id) if surface
            .panes
            .iter()
            .any(|pane| &pane.pane_id == id && pane.alternate_screen_active));
    Some(WheelTarget {
        target,
        mouse_reporting,
        stepwise,
        bounds: Bounds::new(
            point(px(origin_x), px(origin_y)),
            size(
                px(rect.width as f32 * cell_width),
                px(rect.height as f32 * cell_height),
            ),
        ),
        position,
        geometry,
    })
}

#[cfg(test)]
mod tests;
