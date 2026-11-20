//! L9: L0 placement on the real screen, too-small window, keyboards.
//! Owner: L9.
//!
//! The terminal receives only L0's rect. When L0 says the window cannot hold
//! one pane at 280×206 (Q1, D2), the screen says "Window too small" in place
//! of the terminal, and neither a size nor input reaches the terminal until
//! the window grows again (`TerminalInput::hold_for_size`).
//!
//! The status bar moves its optional items to the details as it narrows:
//! Hardware keyboard first, then the M5 sections; the connection name (cut
//! with an ellipsis), Mods and Shortcuts stay (design text, "状態バーは").
//!
//! A floating (undocked) keyboard never resizes the terminal; only a keyboard
//! docked along the window's bottom edge is an L0 inset. The edit bar and the
//! IME candidates move clear of a floating keyboard instead
//! (`place_clear_of_keyboard`).
use super::disconnect::DisconnectClock;
use crate::{
    app::Smoke,
    layout::{
        ControlsClearance, Rect, Regions, Size, TERMINAL_MIN_HEIGHT, TERMINAL_MIN_WIDTH,
        TerminalFit, TooSmall,
    },
};
use gpui::{AnyElement, Context, Edges, Pixels, Window, div, prelude::*, px, rgb};

/// The title shown in place of the terminal.
pub(crate) const TOO_SMALL_TITLE: &str = "Window too small";

/// Width the "Hardware keyboard" item takes in the status bar: the 12 pt gap
/// before it, an 18 pt icon, a 6 pt gap and the 13 pt label (Main.dc.html).
pub(crate) const HARDWARE_KEYBOARD_WIDTH: f32 = 152.0;
/// Gap between status bar items (Main.dc.html `gap: 12px`).
pub(crate) const STATUS_GAP: f32 = 12.0;

#[derive(Debug, Default)]
pub(crate) struct FitState {
    /// L0's verdict in the last frame; `Some` while the window is too small.
    pub(crate) too_small: Option<TooSmall>,
    /// The Mods list is open in overlay O (`mods.rs`).
    pub(crate) mods_open: bool,
    /// Mod regions the mod runtime asks for. Empty until the runtime lands;
    /// Simulator builds take a fixed request from `YEBISU_LAYOUT_MODS_QA`.
    pub(crate) mod_regions: Regions,
    /// When the line dropped; mod regions go to size 0 after 30 s.
    pub(crate) disconnect: DisconnectClock,
    /// A hardware key (`is_hardware_key`) reached the window since the
    /// software keyboard last showed docked (the status bar's Hardware
    /// keyboard item, and the filter field's focus).
    pub(crate) hardware_keyboard: bool,
    /// The window's key presses are watched for `is_hardware_key`.
    pub(crate) watching_keys: bool,
    /// The window controls' reach (left, right) and L0's clearance last
    /// written to the observation log; a record is written only on change.
    pub(crate) recorded_controls: Option<(f32, f32, ControlsClearance)>,
    /// Tests move the fit clock ahead without waiting.
    #[cfg(test)]
    pub(crate) clock_ahead: std::time::Duration,
    /// Tests give the window these platform insets (GPUI's test platform
    /// cannot change them from outside the crate).
    #[cfg(test)]
    pub(crate) insets: Option<gpui::WindowInsets>,
    /// Tests give the window this window-controls reach (herdr-uikit is
    /// iOS-only).
    #[cfg(test)]
    pub(crate) window_controls: Option<Edges<Pixels>>,
}

/// Status bar items in the order they move to the details popover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StatusItem {
    HardwareKeyboard,
    ModSections,
}

/// The order items leave the status bar as it narrows (design text).
pub(crate) const OVERFLOW_ORDER: [StatusItem; 2] =
    [StatusItem::HardwareKeyboard, StatusItem::ModSections];

/// Items moved to the details when the status bar has `available_width`
/// beside the items that always stay (status dot, connection name at its
/// shortest, Mods, Shortcuts). No mod runs yet, so the M5 sections take no
/// width here; `status_bar_overflow_with` takes their width explicitly.
pub(crate) fn status_bar_overflow(available_width: f32) -> Vec<StatusItem> {
    status_bar_overflow_with(available_width, 0.0)
}

/// As `status_bar_overflow`, with the M5 sections `m5_width` wide (0 when no
/// section is shown). Items leave in `OVERFLOW_ORDER` until the rest fit.
pub(crate) fn status_bar_overflow_with(available_width: f32, m5_width: f32) -> Vec<StatusItem> {
    let width = |item: StatusItem| match item {
        StatusItem::HardwareKeyboard => HARDWARE_KEYBOARD_WIDTH,
        StatusItem::ModSections if m5_width > 0.0 => m5_width + STATUS_GAP,
        StatusItem::ModSections => 0.0,
    };
    let mut needed: f32 = OVERFLOW_ORDER.iter().copied().map(width).sum();
    let mut moved = Vec::new();
    for item in OVERFLOW_ORDER {
        if needed <= available_width.max(0.0) {
            break;
        }
        let item_width = width(item);
        if item_width > 0.0 {
            moved.push(item);
        }
        needed -= item_width;
    }
    moved
}

/// The message under the title: what the terminal needs and what it has.
pub(crate) fn too_small_message(too_small: &TooSmall) -> String {
    format!(
        "The terminal needs at least {:.0} × {:.0} pt; this window leaves {:.0} × {:.0} pt. \
         Make the window larger.",
        too_small.minimum.width,
        too_small.minimum.height,
        too_small.available.width.max(0.0),
        too_small.available.height.max(0.0),
    )
}

/// L0's verdict for the terminal: `Some` when the window is too small.
pub(crate) fn too_small(fit: TerminalFit) -> Option<TooSmall> {
    match fit {
        TerminalFit::Fits(rect)
            if rect.width >= TERMINAL_MIN_WIDTH && rect.height >= TERMINAL_MIN_HEIGHT =>
        {
            None
        }
        TerminalFit::Fits(rect) => Some(TooSmall {
            available: rect.size(),
            minimum: Size::new(TERMINAL_MIN_WIDTH, TERMINAL_MIN_HEIGHT),
        }),
        TerminalFit::TooSmall(too_small) => Some(too_small),
    }
}

/// The keyboard height L0 takes as the bottom inset. Only a keyboard docked
/// along the bottom edge across the whole window width counts; a floating
/// (undocked or split) keyboard returns 0, so it never resizes the terminal.
pub(crate) fn docked_keyboard_height(window: Size, keyboard: Option<Rect>) -> f32 {
    let Some(keyboard) = keyboard else {
        return 0.0;
    };
    let at_bottom = (keyboard.bottom() - window.height).abs() < 1.0;
    let full_width = keyboard.x <= 0.5 && keyboard.right() >= window.width - 0.5;
    if at_bottom && full_width && keyboard.height > 0.0 {
        keyboard.height.min(window.height)
    } else {
        0.0
    }
}

/// The keyboard as a rect from the platform's keyboard insets, `None` when
/// the platform reports none. A keyboard inset from the left or right edge
/// does not span the window, so `docked_keyboard_height` takes it as floating.
pub(crate) fn ime_keyboard(window: Size, ime: &Edges<Pixels>) -> Option<Rect> {
    let height = f32::from(ime.bottom).min(window.height);
    if height <= 0.0 {
        return None;
    }
    let left = f32::from(ime.left).max(0.0);
    let right = f32::from(ime.right).max(0.0);
    Some(Rect::new(
        left,
        window.height - height,
        (window.width - left - right).max(0.0),
        height,
    ))
}

/// Whether `keystroke` can only have come from a hardware keyboard. The
/// software keyboard types characters as text, not key presses; the presses
/// it does send (herdr-uikit 99d7928 `tap_key`, from `keymap::control_keystroke`
/// and `keymap::backspace`) are Return (`"\n"`), Tab (`"\t"`), Escape,
/// Backspace and Control-letters, none with a printable character. A press
/// with a printable character and no Control, Command or Fn is therefore a
/// physical key; Shift and Option are allowed.
pub(crate) fn is_hardware_key(keystroke: &gpui::Keystroke) -> bool {
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.platform || modifiers.function {
        return false;
    }
    keystroke
        .key_char
        .as_deref()
        .is_some_and(|text| text.chars().any(|c| !c.is_control()))
}

/// Where `item` (the edit bar, or the IME candidate bar) goes so a floating
/// keyboard does not cover it: unchanged when they do not overlap, else just
/// above the keyboard, else just below it, inside `bounds`. `None` when
/// neither side has room: the same actions go to the standard edit menu.
pub(crate) fn place_clear_of_keyboard(item: Rect, keyboard: Rect, bounds: Rect) -> Option<Rect> {
    if !item.overlaps(keyboard) {
        return Some(item);
    }
    let above = keyboard.y - item.height;
    if above >= bounds.y {
        return Some(Rect::new(item.x, above, item.width, item.height));
    }
    let below = keyboard.bottom();
    if below + item.height <= bounds.bottom() {
        return Some(Rect::new(item.x, below, item.width, item.height));
    }
    None
}

impl Smoke {
    /// "Window too small" in place of the terminal; `None` draws as today.
    pub(crate) fn render_too_small(
        &mut self,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let verdict = too_small(self.frame_layout(window).terminal);
        self.shell.fit.too_small = verdict;
        self.terminal.input.hold_for_size(verdict.is_some());
        let verdict = verdict?;
        Some(
            div()
                .id("shell-too-small")
                .debug_selector(|| "shell-too-small".into())
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .px_4()
                .bg(rgb(0x101419))
                .text_color(rgb(0xd8dee9))
                .child(div().text_lg().child(TOO_SMALL_TITLE))
                .child(
                    div()
                        .max_w(px(420.))
                        .text_sm()
                        .text_color(rgb(0xacb2bb))
                        .child(too_small_message(&verdict)),
                )
                .into_any_element(),
        )
    }

    /// The platform's insets for this window (safe area and keyboard).
    pub(crate) fn platform_insets(&self, window: &Window) -> gpui::WindowInsets {
        #[cfg(test)]
        if let Some(insets) = &self.shell.fit.insets {
            return insets.clone();
        }
        window.insets()
    }

    /// How far iPadOS 26's window controls reach into this window past its
    /// safe area, per physical edge, in points. herdr-uikit samples UIKit's
    /// corner-adapted safe area on layout callbacks and redraws the window
    /// when it changes; zero before iPadOS 26 and on other platforms.
    pub(crate) fn platform_window_controls(&self, window: &Window) -> Edges<Pixels> {
        #[cfg(test)]
        if let Some(controls) = self.shell.fit.window_controls {
            return controls;
        }
        #[cfg(target_os = "ios")]
        {
            herdr_uikit::window_control_insets(window.window_handle())
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = window;
            Edges::default()
        }
    }

    /// Writes `{"kind":"window_controls"}` with UIKit's reach and L0's
    /// clearance when either changes (QA evidence of what the platform
    /// reported).
    pub(crate) fn record_window_controls(
        &mut self,
        reach: Edges<Pixels>,
        clearance: ControlsClearance,
    ) {
        let now = (f32::from(reach.left), f32::from(reach.right), clearance);
        if self.shell.fit.recorded_controls == Some(now) {
            return;
        }
        self.shell.fit.recorded_controls = Some(now);
        self.observation.record(serde_json::json!({
            "kind": "window_controls",
            "left": now.0,
            "right": now.1,
            "header_leading": clearance.header_leading,
            "rail_top": clearance.rail_top,
        }));
    }

    /// Watches every key press of the window once, so a press only a
    /// hardware keyboard sends (`is_hardware_key`) marks one in use.
    pub(crate) fn watch_hardware_keys(&mut self, cx: &mut Context<Self>) {
        if self.shell.fit.watching_keys {
            return;
        }
        self.shell.fit.watching_keys = true;
        let weak = cx.entity().downgrade();
        cx.intercept_keystrokes(move |event, _, app| {
            if !is_hardware_key(&event.keystroke) {
                return;
            }
            // The view may be gone or busy; the next press tries again.
            weak.update(app, |this, cx| {
                if !this.shell.fit.hardware_keyboard {
                    this.shell.fit.hardware_keyboard = true;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The clock of the mod regions' 30 s fade.
    pub(crate) fn fit_now(&self) -> std::time::Instant {
        let now = std::time::Instant::now();
        #[cfg(test)]
        let now = now + self.shell.fit.clock_ahead;
        now
    }

    /// False while the window is too small: no size or input reaches the
    /// terminal.
    pub(crate) fn terminal_send_allowed(&self) -> bool {
        self.shell.fit.too_small.is_none()
    }
}

#[cfg(test)]
#[path = "fit_tests.rs"]
mod tests;
