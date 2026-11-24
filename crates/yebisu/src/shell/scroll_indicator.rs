//! Issue #137 PR 5: a thin thumb at a list's right edge while it scrolls and
//! for 1.0 s after (the design is written in the issue; there is no canvas
//! file). GPUI's scroll containers draw no indicator, so each list keeps one
//! of these: the list tracks its `ScrollHandle`, and a zero-size canvas
//! inside the list paints the thumb at the list's own bounds, which do not
//! scroll. The thumb is no accessibility element.

use crate::shell::style::color;
use gpui::{
    BorderStyle, Bounds, Pixels, Point, ScrollHandle, canvas, point, prelude::*, px, quad, rgba,
    size,
};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

/// How long the thumb stays after the list last moved.
pub(crate) const VISIBLE: Duration = Duration::from_millis(1000);
const WIDTH: f32 = 3.;
/// From the list's right edge.
const RIGHT: f32 = 2.;
/// Above and below the thumb's track.
const INSET: f32 = 2.;
const RADIUS: f32 = 1.5;
const MIN_LENGTH: f32 = 36.;
/// `color::MUTED` at 75 % (the terminal thumb's `SCROLLBAR_ALPHA`).
const ALPHA: u32 = 0xc0;

/// The thumb's top within the list and its length, or none when the content
/// fits: `viewport` tall, scrollable by `max`, scrolled `scrolled` down.
pub(crate) fn thumb_geometry(viewport: f32, max: f32, scrolled: f32) -> Option<(f32, f32)> {
    if !(viewport > 0. && max > 0.) {
        return None;
    }
    let track = (viewport - 2. * INSET).max(0.);
    let length = (viewport * viewport / (viewport + max))
        .max(MIN_LENGTH)
        .min(track);
    let top = INSET + (track - length) * (scrolled / max).clamp(0., 1.);
    Some((top, length))
}

/// Whether a list that last moved at `moved` still shows its thumb at `now`.
pub(crate) fn visible(moved: Instant, now: Instant) -> bool {
    now.saturating_duration_since(moved) < VISIBLE
}

/// A list's scroll offset and when it changed to it.
type Seen = (Point<Pixels>, Instant);

/// One list's thumb: the handle the list tracks, when it last moved, and
/// what was painted last (for tests).
#[derive(Clone, Debug, Default)]
pub(crate) struct ScrollIndicator {
    handle: ScrollHandle,
    /// The offset last seen and when it changed to it.
    seen: Rc<Cell<Option<Seen>>>,
    painted: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl ScrollIndicator {
    /// The handle the list tracks (`track_scroll`).
    pub(crate) fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    /// The thumb painted last, if one was showing.
    pub(crate) fn painted(&self) -> Option<Bounds<Pixels>> {
        self.painted.get()
    }

    /// A child for the list that paints the thumb over its content.
    pub(crate) fn thumb(&self) -> impl IntoElement + use<> {
        let this = self.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let now = Instant::now();
                let offset = this.handle.offset();
                // A list's first paint is not a scroll; a change of offset is.
                let moved = match this.seen.get() {
                    Some((seen, at)) if seen == offset => at,
                    Some(_) => now,
                    None => now.checked_sub(VISIBLE).unwrap_or(now),
                };
                this.seen.set(Some((offset, moved)));
                let list = this.handle.bounds();
                let geometry = thumb_geometry(
                    f32::from(list.size.height),
                    f32::from(this.handle.max_offset().y),
                    -f32::from(offset.y),
                )
                .filter(|_| visible(moved, now));
                let Some((top, length)) = geometry else {
                    this.painted.set(None);
                    return;
                };
                let bounds = Bounds::new(
                    point(list.right() - px(RIGHT + WIDTH), list.top() + px(top)),
                    size(px(WIDTH), px(length)),
                );
                window.paint_quad(quad(
                    bounds,
                    px(RADIUS),
                    rgba((color::MUTED << 8) | ALPHA),
                    px(0.),
                    rgba(0),
                    BorderStyle::default(),
                ));
                this.painted.set(Some(bounds));
                // Paint again to hide it once the list has rested for VISIBLE.
                window.request_animation_frame();
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_0()
    }
}

/// One indicator per list that scrolls (issue #137 PR 5, C1).
#[derive(Clone, Debug, Default)]
pub(crate) struct Indicators {
    pub(crate) sidebar: ScrollIndicator,
    pub(crate) rail: ScrollIndicator,
    pub(crate) settings_categories: ScrollIndicator,
    pub(crate) settings_content: ScrollIndicator,
    pub(crate) first_run: ScrollIndicator,
    pub(crate) hosts_list: ScrollIndicator,
    pub(crate) add_host: ScrollIndicator,
    pub(crate) nearby: ScrollIndicator,
    pub(crate) shortcuts: ScrollIndicator,
    pub(crate) overflow_menu: ScrollIndicator,
    pub(crate) new_space: ScrollIndicator,
    pub(crate) compact_sidebar: ScrollIndicator,
    pub(crate) compact_tabs: ScrollIndicator,
}

#[cfg(test)]
#[path = "scroll_indicator_tests.rs"]
mod tests;
