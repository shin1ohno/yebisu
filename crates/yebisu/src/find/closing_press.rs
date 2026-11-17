//! The Escape press that closed search belongs to the UI until it ends.
//!
//! Closing search retires the native text view: herdr-uikit 99d7928 installs
//! a new view without the old one's press or repeat state
//! (window/text_input/retirement.rs:59-95), and a `KeyDownEvent` carries no
//! press identity (the window press ledger, window/state/press_ledger.rs, is
//! crate-private). So a KeyDown of the still-held closing key can arrive with
//! `is_held == false`, and `is_held` cannot tell it from a fresh press. The
//! `consumed` set in terminal_controls.rs gives up on any non-held KeyDown
//! (it recovers presses whose release was lost), which let such a KeyDown
//! reach terminal input on device (P13 device check, step 18).
//!
//! This latch swallows every KeyDown and the KeyUp of the closing key until
//! the press ends: a KeyUp of that key, or on iOS the window's press ledger
//! holding no press at all. A release the ledger never sees keeps the latch
//! until the next press of that key is released, so at most that one press
//! is swallowed; none reaches the terminal.
use crate::app::Smoke;
use gpui::AnyWindowHandle;

pub(crate) struct ClosingPress {
    key: String,
    #[cfg_attr(
        not(target_os = "ios"),
        allow(dead_code, reason = "only the iOS press ledger reads the window")
    )]
    window: AnyWindowHandle,
}
impl ClosingPress {
    pub(crate) fn new(key: &str, window: AnyWindowHandle) -> Self {
        Self {
            key: key.to_owned(),
            window,
        }
    }
    /// Whether the window holds no press, so the closing press has ended.
    /// A closed window (`None`) delivers no further keys either.
    #[cfg(target_os = "ios")]
    fn released(&self) -> bool {
        herdr_uikit::native_hardware_hold(self.window).is_none_or(|hold| !hold.held())
    }
}
impl Smoke {
    /// Whether a KeyDown of `key`, held or not, belongs to the closing press.
    pub(crate) fn closing_press_owns(&self, key: &str) -> bool {
        self.find_handoff
            .closing_press
            .as_ref()
            .is_some_and(|press| press.key == key)
    }
    /// Ends the closing press on its KeyUp. Returns whether this KeyUp was
    /// that press's, so the caller swallows it.
    pub(crate) fn end_closing_press(&mut self, key: &str) -> bool {
        if !self.closing_press_owns(key) {
            return false;
        }
        self.find_handoff.closing_press = None;
        self.actions.consumed.remove(key);
        true
    }
    /// Ends the closing press once the window's press ledger holds nothing:
    /// its release may have gone to the retired native view, which no longer
    /// dispatches to GPUI.
    #[cfg(target_os = "ios")]
    pub(crate) fn poll_closing_press(&mut self) {
        if self
            .find_handoff
            .closing_press
            .as_ref()
            .is_some_and(ClosingPress::released)
        {
            self.find_handoff.closing_press = None;
        }
    }
}
