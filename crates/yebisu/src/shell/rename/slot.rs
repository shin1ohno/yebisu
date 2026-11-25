//! Where the field goes (issue #143 B-5): the element a rename covers
//! reports its drawn rectangle and the visible area around it every frame
//! (`slot_canvas`), and `rename_frame_tick` moves the field when it
//! changed, presents it once the anchor has been drawn, and closes it when
//! the anchor or the object went away (Q-R5, B-9).
use super::{Phase, RenameAnchor, RenameObject};
use crate::{
    app::Smoke,
    board::snapshot,
    shell::style::{text, weight},
};
use gpui::{Bounds, Context, IntoElement, Pixels, Styled, Window, canvas};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// The anchor's rectangle and the visible area it was drawn in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placed {
    pub(crate) frame: Bounds<Pixels>,
    pub(crate) clip: Option<Bounds<Pixels>>,
}

#[derive(Default)]
struct Drawn {
    /// Incremented by each `rename_frame_tick`, before the frame's elements
    /// are laid out: a report carries the frame it was drawn in.
    frame: u64,
    at: HashMap<(RenameAnchor, RenameObject), (Placed, u64)>,
}

/// Each anchor's last drawn rectangle, shared with the canvases.
#[derive(Clone, Default)]
pub(crate) struct Slots(Rc<RefCell<Drawn>>);

impl Slots {
    /// `anchor` of `object` was drawn at `frame` inside `clip` this frame.
    pub(crate) fn report(
        &self,
        anchor: RenameAnchor,
        object: &RenameObject,
        frame: Bounds<Pixels>,
        clip: Bounds<Pixels>,
    ) {
        let mut drawn = self.0.borrow_mut();
        let now = drawn.frame;
        drawn.at.insert(
            (anchor, object.clone()),
            (
                Placed {
                    frame,
                    clip: Some(clip),
                },
                now,
            ),
        );
    }

    fn begin_frame(&self) -> u64 {
        let mut drawn = self.0.borrow_mut();
        drawn.frame += 1;
        drawn.frame
    }

    /// Where `anchor` was drawn in the frame before `now`, if it was.
    fn drawn_before(
        &self,
        anchor: RenameAnchor,
        object: &RenameObject,
        now: u64,
    ) -> Option<Placed> {
        self.0
            .borrow()
            .at
            .get(&(anchor, object.clone()))
            .filter(|(_, frame)| frame + 1 == now)
            .map(|(placed, _)| *placed)
    }

    /// Where `anchor` was drawn last (tests and QA).
    pub(crate) fn last(&self, anchor: RenameAnchor, object: &RenameObject) -> Option<Placed> {
        self.0
            .borrow()
            .at
            .get(&(anchor, object.clone()))
            .map(|(placed, _)| *placed)
    }
}

/// A canvas filling the element it is a child of, reporting that element's
/// bounds and the content mask (the scroll area's visible part).
pub(crate) fn slot_canvas(
    slots: Slots,
    anchor: RenameAnchor,
    object: RenameObject,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            slots.report(anchor, &object, bounds, window.content_mask().bounds);
        },
        |_, (), _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The field's text size and weight (UIKit's `UIFontWeight`) for the label
/// it covers.
pub(crate) fn font(anchor: RenameAnchor) -> (f32, f32) {
    let bold = |w: gpui::FontWeight| if w.0 >= weight::SEMIBOLD.0 { 0.4 } else { 0.0 };
    match anchor {
        RenameAnchor::SpaceRow | RenameAnchor::AgentRow => (text::NAME, 0.0),
        RenameAnchor::TabLabel => (text::BASE, 0.0),
        RenameAnchor::RailHeader => (text::BASE, bold(weight::SEMIBOLD)),
        RenameAnchor::PaneChip => (text::NAME, bold(weight::BOLD)),
    }
}

/// Room kept between a revealed row's field and the list's edge.
const REVEAL_MARGIN: f32 = 8.;

/// What the frame tick decided, run after the render that decided it: the
/// SDK is never called inside a render.
enum Step {
    Present(u64, Placed),
    Move(Placed),
    Close(&'static str),
    Unavailable(u64),
}

impl Smoke {
    /// Once per drawn frame, before the frame's elements: settle a pending
    /// name, then follow, present or close the open field.
    pub(crate) fn rename_frame_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = self.shell.rename.slots.begin_frame();
        self.settle_pending_rename();
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        super::qa::publish(
            window.window_handle(),
            super::qa::snapshot(self),
            &self.observation,
        );
        let Some(step) = self.rename_step(now) else {
            // The anchor is drawn from this frame on: the next one presents.
            if self
                .shell
                .rename
                .session
                .as_ref()
                .is_some_and(|s| s.phase == Phase::Measuring)
            {
                window.request_animation_frame();
            }
            return;
        };
        cx.defer_in(window, move |this, window, cx| {
            match step {
                Step::Present(id, placed) => this.present_rename(id, placed, window, cx),
                Step::Move(placed) => this.move_rename(placed),
                Step::Close(reason) => this.dismiss_rename(reason),
                Step::Unavailable(id) => {
                    if this.shell.rename.session.as_ref().map(|s| s.id) == Some(id) {
                        this.dismiss_rename(super::UNAVAILABLE_HERE);
                    }
                }
            }
            cx.notify();
        });
    }

    fn rename_step(&mut self, now: u64) -> Option<Step> {
        let session = self.shell.rename.session.as_ref()?;
        if session.phase == Phase::Restoring || session.closing {
            return None;
        }
        let (anchor, object) = (session.anchor, session.target.object.clone());
        let live = &self.terminal.live;
        let gone = if !live.connected || self.terminal.reconnect.target != session.target.host {
            Some(super::DISCONNECTED)
        } else {
            match snapshot(live) {
                None => Some(super::DISCONNECTED),
                Some(s) if s.boot_id != session.target.boot || !object.exists_in(s) => {
                    Some(object.closed_text())
                }
                Some(_) => None,
            }
        };
        if let Some(reason) = gone {
            return Some(Step::Close(reason));
        }
        let drawn = self.shell.rename.slots.drawn_before(anchor, &object, now);
        let session = self.shell.rename.session.as_mut()?;
        let Some(placed) = drawn else {
            session.missed = session.missed.saturating_add(1);
            // The first tick after `begin_rename` comes before any frame drew
            // the anchor (it is drawn only while it is being renamed); after
            // that, two frames in a row without it end the session.
            let limit = if session.phase == Phase::Measuring { 3 } else { 2 };
            if session.missed >= limit {
                return Some(if session.phase == Phase::Measuring {
                    Step::Unavailable(session.id)
                } else {
                    Step::Close(super::CANCELLED)
                });
            }
            return None;
        };
        session.missed = 0;
        match session.phase {
            Phase::Measuring => Some(Step::Present(session.id, placed)),
            Phase::Presenting | Phase::Open if session.placed != Some(placed) => {
                session.placed = Some(placed);
                Some(Step::Move(placed))
            }
            _ => None,
        }
    }

    fn move_rename(&mut self, placed: Placed) {
        self.reveal_rename_row(placed);
        let Some(control) = self
            .shell
            .rename
            .session
            .as_ref()
            .and_then(|s| s.control.as_ref())
        else {
            return;
        };
        if let Err(error) = control.set_frame(placed.frame, placed.clip) {
            tracing::warn!(?error, "rename field not moved");
        }
    }

    /// A sidebar row whose field is (partly) outside the list's visible
    /// area, a docked keyboard's included, scrolls the list to show it.
    fn reveal_rename_row(&mut self, placed: Placed) {
        let in_sidebar = self.shell.rename.session.as_ref().is_some_and(|s| {
            matches!(s.anchor, RenameAnchor::SpaceRow | RenameAnchor::AgentRow)
        });
        let Some(clip) = placed.clip.filter(|_| in_sidebar) else {
            return;
        };
        let frame = placed.frame;
        let below = frame.bottom() - clip.bottom();
        let above = clip.top() - frame.top();
        let shift = if below > gpui::px(0.) {
            -(below + gpui::px(REVEAL_MARGIN))
        } else if above > gpui::px(0.) {
            above + gpui::px(REVEAL_MARGIN)
        } else {
            return;
        };
        let scroll = &self.shell.rename.sidebar_scroll;
        let mut offset = scroll.offset();
        offset.y += shift;
        scroll.set_offset(offset);
    }
}
