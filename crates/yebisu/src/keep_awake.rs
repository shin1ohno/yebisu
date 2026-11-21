//! Keeps the iPad display awake while a host is connected (R54).
//!
//! GPUI's `prevent_idle_sleep` is herdr-uikit's hook onto UIKit's
//! process-wide idle timer (`platform/idle.rs` at the pinned revision): while
//! a guard lives the screen neither dims nor locks, and dropping the last one
//! hands the idle timer back to the system. The fork held it from a manual
//! toggle (herdr-gpui e907fe9cb, `caffeine/ios.rs`); yebisu holds it exactly
//! while some host's status is Connected and the app is in front, so a
//! session someone is watching does not lock, and a backgrounded or
//! disconnected app never keeps the screen lit.
use crate::app::Smoke;
use gpui::{ActivityGuard, AppLifecyclePhase, Context};

/// Kept in herdr-uikit's debug log; the system keeps no record of it.
const REASON: &str = "yebisu: a host is connected";

#[derive(Default)]
pub(crate) struct KeepAwake {
    /// Backgrounded, or the scene disconnected: no hold, whatever is connected.
    background: bool,
    hold: Hold,
    /// Fences a guard that arrives after the attempt it answers was released.
    attempts: u64,
}

#[derive(Default)]
enum Hold {
    #[default]
    Released,
    /// Asked of the platform under this attempt; its guard has not arrived.
    Pending(u64),
    /// Kept only to be dropped: that releases the idle timer.
    Held { _guard: ActivityGuard },
    /// The platform refused. It is asked again only once the hold has been
    /// unwanted and wanted again, so a lasting refusal is reported once
    /// rather than on every poll.
    Refused,
}

/// What the platform must do so the hold matches what is wanted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Keep,
    Acquire(u64),
    Release,
}

/// How the platform's answer to an attempt was taken.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Acquired {
    Held,
    /// The attempt was released before the answer came; its guard is dropped.
    Stale,
}

impl KeepAwake {
    /// Inactive (Control Center, the app switcher) keeps the hold, as it keeps
    /// both transports (`reconnect.rs`).
    pub(crate) fn lifecycle(&mut self, phase: AppLifecyclePhase) {
        match phase {
            AppLifecyclePhase::Background | AppLifecyclePhase::Disconnected => {
                self.background = true;
            }
            AppLifecyclePhase::Foreground | AppLifecyclePhase::Active => self.background = false,
            AppLifecyclePhase::Inactive => {}
        }
    }

    /// The hold is wanted while the app is in front and any host is connected.
    pub(crate) fn step(&mut self, connected: impl IntoIterator<Item = bool>) -> Step {
        let wanted = !self.background && connected.into_iter().any(|connected| connected);
        match (&self.hold, wanted) {
            (Hold::Released, true) => {
                self.attempts = self.attempts.wrapping_add(1);
                self.hold = Hold::Pending(self.attempts);
                Step::Acquire(self.attempts)
            }
            (Hold::Pending(_) | Hold::Held { .. }, false) => {
                self.hold = Hold::Released;
                Step::Release
            }
            (Hold::Refused, false) => {
                self.hold = Hold::Released;
                Step::Keep
            }
            _ => Step::Keep,
        }
    }

    /// Takes the platform's answer to `attempt`. An error means the hold was
    /// refused; it is not asked for again until wanted afresh.
    pub(crate) fn acquired(
        &mut self,
        attempt: u64,
        guard: gpui::Result<ActivityGuard>,
    ) -> gpui::Result<Acquired> {
        if !matches!(self.hold, Hold::Pending(pending) if pending == attempt) {
            return Ok(Acquired::Stale);
        }
        match guard {
            Ok(guard) => {
                self.hold = Hold::Held { _guard: guard };
                Ok(Acquired::Held)
            }
            Err(error) => {
                self.hold = Hold::Refused;
                Err(error)
            }
        }
    }
}

impl Smoke {
    /// Runs on every UI poll and lifecycle phase; it asks the platform only
    /// when the wanted hold changes.
    pub(crate) fn sync_keep_awake(&mut self, cx: &mut Context<Self>) {
        let connected = self
            .board
            .terminals(&self.terminal)
            .map(|terminal| terminal.live.connected);
        match self.keep_awake.step(connected) {
            Step::Keep => {}
            Step::Release => crate::diagnostics::record("keep_awake_off"),
            Step::Acquire(attempt) => {
                let guard = cx.prevent_idle_sleep(REASON);
                cx.spawn(async move |this, cx| {
                    let guard = guard.await;
                    // A dropped view drops the guard with this closure.
                    let _ = this.update(cx, |this, _| {
                        match this.keep_awake.acquired(attempt, guard) {
                            Ok(Acquired::Held) => crate::diagnostics::record("keep_awake_on"),
                            Ok(Acquired::Stale) => {}
                            Err(error) => {
                                crate::diagnostics::record("keep_awake_refused");
                                tracing::warn!("keeping the display awake was refused: {error:#}");
                            }
                        }
                    });
                })
                .detach();
            }
        }
    }
}

#[cfg(test)]
mod tests;
