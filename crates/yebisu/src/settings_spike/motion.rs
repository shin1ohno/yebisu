// Modified from https://github.com/shin1ohno/herdr-gpui crates/sheprd-gpui/src/motion.rs at 398c132a06723a0cc1494842a34666d173ec0200
use std::time::{Duration, Instant};
/// How long something takes to grow in.
pub(crate) const ENTER: Duration = Duration::from_millis(180);
/// How long something takes to shrink out; leaving is a little quicker.
pub(crate) const LEAVE: Duration = Duration::from_millis(150);

/// How far a motion that began at `since` has come at `now`, eased out, or
/// `None` once it is done.
pub(crate) fn progress(since: Instant, now: Instant, length: Duration) -> Option<f32> {
    let k = now.saturating_duration_since(since).as_secs_f32() / length.as_secs_f32();
    (k < 1.).then(|| 1. - (1. - k.clamp(0., 1.)).powi(3))
}

/// Something that opens and closes, such as a panel: how much of it shows.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Toggle {
    open: bool,
    /// When it last opened or closed; `None` before either, so a panel
    /// that is open when first drawn is simply there.
    since: Option<Instant>,
}

impl Toggle {
    /// Opens or closes it at `now`. The first report sets where it is
    /// without moving, as a view first drawn shows what it has.
    pub(crate) fn set(&mut self, open: bool, now: Instant, first: bool) {
        if first {
            *self = Self { open, since: None };
            return;
        }
        if open != self.open {
            // Reversing mid-way starts from where it is, not from an end.
            let shown = self.shown(now);
            let length = if open { ENTER } else { LEAVE };
            let done = if open { shown } else { 1. - shown };
            // Inverts the ease: the time at which it would have come this far.
            let back = length.mul_f32(1. - (1. - done.clamp(0., 1.)).cbrt());
            self.open = open;
            self.since = Some(now.checked_sub(back).unwrap_or(now));
        }
    }

    /// Whether it is open or opening, rather than closed or closing.
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// How much of it shows at `now`, from 0 closed to 1 open.
    pub(crate) fn shown(&self, now: Instant) -> f32 {
        let length = if self.open { ENTER } else { LEAVE };
        let k = self
            .since
            .and_then(|since| progress(since, now, length))
            .unwrap_or(1.);
        if self.open { k } else { 1. - k }
    }

    /// Whether it is still opening or closing.
    pub(crate) fn moving(&self, now: Instant) -> bool {
        let length = if self.open { ENTER } else { LEAVE };
        self.since
            .is_some_and(|since| progress(since, now, length).is_some())
    }
}
