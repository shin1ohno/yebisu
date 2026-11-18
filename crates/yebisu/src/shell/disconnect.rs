//! L9: the 30 s fade of a disconnected region (mod point 7). Owner: L9.
//!
//! When the line drops, a mod region stops taking input, keeps its last
//! screen dimmed for 30 s, then goes to size 0. Drafts stay (nothing here
//! clears them). The clock is passed in, so tests replace it.
use std::time::{Duration, Instant};

/// How long the last screen stays, dimmed, after the line drops.
pub(crate) const DISCONNECT_HOLD: Duration = Duration::from_secs(30);
/// Opacity of the dimmed last screen.
pub(crate) const DIMMED_OPACITY: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DisconnectFade {
    Visible,
    Dimmed,
    Cleared,
}

/// The fade `elapsed` after the line dropped: dimmed for 30 s, then cleared.
pub(crate) fn disconnect_fade(elapsed: Duration) -> DisconnectFade {
    if elapsed < DISCONNECT_HOLD {
        DisconnectFade::Dimmed
    } else {
        DisconnectFade::Cleared
    }
}

impl DisconnectFade {
    /// Only a connected region takes taps, drags and input.
    pub(crate) fn interactive(self) -> bool {
        self == Self::Visible
    }

    /// The region's opacity; a cleared region also takes no space.
    pub(crate) fn opacity(self) -> f32 {
        match self {
            Self::Visible => 1.0,
            Self::Dimmed => DIMMED_OPACITY,
            Self::Cleared => 0.0,
        }
    }

    /// Whether the region keeps its size (a cleared one is 0).
    pub(crate) fn keeps_size(self) -> bool {
        self != Self::Cleared
    }
}

/// When a region's line dropped, against an injected clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DisconnectClock {
    since: Option<Instant>,
}

impl DisconnectClock {
    /// Records the line state at `now`; the first drop starts the clock and
    /// a reconnect stops it.
    pub(crate) fn observe(&mut self, connected: bool, now: Instant) {
        match (connected, self.since) {
            (true, _) => self.since = None,
            (false, None) => self.since = Some(now),
            (false, Some(_)) => {}
        }
    }

    pub(crate) fn fade(&self, now: Instant) -> DisconnectFade {
        match self.since {
            None => DisconnectFade::Visible,
            Some(since) => disconnect_fade(now.saturating_duration_since(since)),
        }
    }
}

#[cfg(test)]
#[path = "disconnect_tests.rs"]
mod tests;
