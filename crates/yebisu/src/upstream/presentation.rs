// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/presentation.rs at 398c132a06723a0cc1494842a34666d173ec0200
//! The surface the terminal shows, and the wheel scrolling drawn between its
//! whole rows (`smooth_scroll`), fed the shown surface's transitions. Only the
//! part yebisu needs: it keeps no stale picture across reconnects.

use crate::upstream::smooth_scroll::{Slide, SmoothScroll};
use sheprd_client::protocol::{PaneSurfaceFrame, SurfaceRect};
use std::{sync::Arc, time::Instant};

#[derive(Default)]
pub(crate) struct Presentation {
    presented: Option<Arc<PaneSurfaceFrame>>,
    /// Wheel scrolling drawn between the daemon's rows.
    pub(crate) scroll: SmoothScroll,
}

impl Presentation {
    /// Records the surface painted now. A new surface tells the smooth
    /// scroll where the daemon put the pane; none forgets the picture.
    pub(crate) fn present(&mut self, surface: Option<&Arc<PaneSurfaceFrame>>) {
        let Some(ready) = surface else {
            self.clear();
            return;
        };
        if let Some(shown) = self.presented.as_ref().filter(|s| !Arc::ptr_eq(s, ready)) {
            self.scroll.observe(shown, ready);
        }
        self.presented = Some(ready.clone());
    }

    /// Records that the OS scrolled `pane_id` by `rows`, up into history being
    /// positive, and returns the lines to send (see `SmoothScroll::wheel`).
    pub(crate) fn wheel(&mut self, pane_id: &str, rows: f32) -> Option<i16> {
        let presented = self.presented.as_ref()?;
        self.scroll.wheel(presented, pane_id, rows, Instant::now())
    }

    /// The slide to paint now, if a pane is drawn between rows.
    pub(crate) fn slide(&mut self, now: Instant) -> Option<Slide> {
        self.scroll.slide(now)
    }

    /// The pane drawn between rows and how far, for hit testing.
    pub(crate) fn offset(&self) -> Option<(SurfaceRect, f32)> {
        self.scroll.offset()
    }

    pub(crate) fn clear(&mut self) {
        self.presented = None;
        self.scroll.clear();
    }
}
