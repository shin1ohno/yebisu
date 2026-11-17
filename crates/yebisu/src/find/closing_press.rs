//! The Escape press that closed search belongs to the UI until it ends.
//!
//! Closing search retires the native text view: herdr-uikit 99d7928 installs
//! a new view without the old one's press or repeat state
//! (window/text_input/retirement.rs:59-95), and a `KeyDownEvent` carries no
//! press identity. So a KeyDown of the still-held closing key can arrive with
//! `is_held == false`, and `is_held` cannot tell it from a fresh press. The
//! `consumed` set in terminal_controls.rs gives up on any non-held KeyDown
//! (it recovers presses whose release was lost), which let such a KeyDown
//! reach terminal input on device (P13 device check, step 18).
//!
//! The latch ends only when the press does: a KeyUp of the key (the SDK sends
//! one for `pressesEnded:` and for `pressesCancelled:`, keys.rs
//! `presses_ended`), or the window losing activation. Until then every repeat
//! is the closing press's. A non-held KeyDown is the closing press only while
//! the window's press ledger still holds a press that began before the close:
//! the ledger keys presses by HID usage and ignores a second Began of a held
//! usage (window/state/press_ledger.rs `apply`), and a retired view still
//! records its releases there (input_target.rs `record_presses`). Otherwise
//! it is a new press, which ends the latch and reaches the terminal. A ledger
//! that empties for a moment therefore never ends the latch by itself.
//!
//! Left open: a release that neither GPUI nor the ledger sees, or another key
//! held since before the close, makes the next press of the key look like the
//! closing one; that press is swallowed until its own KeyUp or a deactivation.
use crate::app::Smoke;
use gpui::{AnyWindowHandle, KeyDownEvent};
use std::time::{Duration, Instant};

pub(crate) struct ClosingPress {
    key: String,
    window: AnyWindowHandle,
    closed_at: Instant,
}
impl ClosingPress {
    pub(crate) fn new(key: &str, window: AnyWindowHandle) -> Self {
        Self {
            key: key.to_owned(),
            window,
            closed_at: Instant::now(),
        }
    }
    /// Whether a non-held KeyDown can still be this press: some press that
    /// began before the close is held. `oldest` is how long the window's
    /// oldest held press has been held; `None` means nothing is held.
    fn began_before_close(&self, oldest: Option<Duration>) -> bool {
        oldest.is_some_and(|age| age > self.closed_at.elapsed())
    }
}

/// How long the window's oldest held press has been held, from the SDK's
/// press ledger. `None` when nothing is held or the window is closed.
#[cfg(all(target_os = "ios", not(test)))]
fn oldest_hold(window: AnyWindowHandle) -> Option<Duration> {
    herdr_uikit::native_hardware_hold(window).and_then(|hold| hold.oldest)
}
#[cfg(all(not(target_os = "ios"), not(test)))]
fn oldest_hold(_: AnyWindowHandle) -> Option<Duration> {
    None
}
#[cfg(test)]
thread_local! {
    /// Host tests stand in for the press ledger, which needs UIKit.
    pub(crate) static TEST_OLDEST_HOLD: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn oldest_hold(_: AnyWindowHandle) -> Option<Duration> {
    TEST_OLDEST_HOLD.with(std::cell::Cell::get)
}

impl Smoke {
    /// Whether this KeyDown belongs to the closing press, so the caller
    /// swallows it. A new press of the key ends the latch and is not owned.
    pub(crate) fn closing_press_owns(&mut self, event: &KeyDownEvent) -> bool {
        let Some(press) = self.find_handoff.closing_press.as_ref() else {
            return false;
        };
        if press.key != event.keystroke.key {
            return false;
        }
        let oldest = oldest_hold(press.window);
        if event.is_held || press.began_before_close(oldest) {
            return true;
        }
        self.find_handoff.closing_press = None;
        false
    }
    /// Ends the closing press on its KeyUp. Returns whether this KeyUp was
    /// that press's, so the caller swallows it.
    pub(crate) fn end_closing_press(&mut self, key: &str) -> bool {
        if !self
            .find_handoff
            .closing_press
            .as_ref()
            .is_some_and(|press| press.key == key)
        {
            return false;
        }
        self.drop_closing_press();
        true
    }
    /// Forgets the closing press: the window lost activation, so its release
    /// may never arrive and the next press of the key is a new one.
    pub(crate) fn drop_closing_press(&mut self) {
        if let Some(press) = self.find_handoff.closing_press.take() {
            self.actions.consumed.remove(&press.key);
        }
    }
}
