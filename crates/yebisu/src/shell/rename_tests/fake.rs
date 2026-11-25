//! A recorded field in place of herdr-uikit's: it keeps every request and
//! call, and the test pushes the session's events.
use crate::shell::rename::{
    FieldControl, FieldError, FieldEvent, FieldEvents, FieldPresenter, FieldRequest, Presented,
};
use gpui::{AnyWindowHandle, Bounds, Pixels};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[derive(Default)]
pub(crate) struct Recorded {
    pub(crate) requests: Vec<FieldRequest>,
    pub(crate) frames: Vec<(Bounds<Pixels>, Option<Bounds<Pixels>>)>,
    pub(crate) dismissed: usize,
    /// The control was dropped (the SDK handle released).
    pub(crate) dropped: bool,
    /// Requests the peer had been sent (`sent`) when the control dropped.
    pub(crate) sent_at_drop: Option<usize>,
    pub(crate) sent: Rc<RefCell<usize>>,
    pub(crate) refuse: Option<FieldError>,
    events: VecDeque<FieldEvent>,
    ended: bool,
    waker: Option<Waker>,
}

#[derive(Clone, Default)]
pub(crate) struct RecordedField(pub(crate) Rc<RefCell<Recorded>>);

impl RecordedField {
    pub(crate) fn push(&self, event: FieldEvent) {
        let mut state = self.0.borrow_mut();
        if matches!(event, FieldEvent::InputRestored(_)) {
            state.ended = true;
        }
        state.events.push_back(event);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
    pub(crate) fn presents(&self) -> usize {
        self.0.borrow().requests.len()
    }
    pub(crate) fn request(&self) -> FieldRequest {
        self.0
            .borrow()
            .requests
            .last()
            .cloned()
            .unwrap_or_else(|| panic!("nothing was presented"))
    }
    pub(crate) fn dismissed(&self) -> usize {
        self.0.borrow().dismissed
    }
    pub(crate) fn dropped(&self) -> bool {
        self.0.borrow().dropped
    }
}

struct Control(Rc<RefCell<Recorded>>);

impl FieldControl for Control {
    fn set_frame(
        &self,
        frame: Bounds<Pixels>,
        clip: Option<Bounds<Pixels>>,
    ) -> Result<(), FieldError> {
        self.0.borrow_mut().frames.push((frame, clip));
        Ok(())
    }
    fn dismiss(&self) -> Result<(), FieldError> {
        self.0.borrow_mut().dismissed += 1;
        Ok(())
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.dropped = true;
        let sent = *state.sent.borrow();
        state.sent_at_drop = Some(sent);
    }
}

struct Events(Rc<RefCell<Recorded>>);

impl FieldEvents for Events {
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<FieldEvent>> {
        let mut state = self.0.borrow_mut();
        if let Some(event) = state.events.pop_front() {
            return Poll::Ready(Some(event));
        }
        if state.ended {
            return Poll::Ready(None);
        }
        state.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl FieldPresenter for RecordedField {
    fn present(
        &mut self,
        _window: AnyWindowHandle,
        request: FieldRequest,
    ) -> Result<Presented, FieldError> {
        let mut state = self.0.borrow_mut();
        if let Some(error) = state.refuse {
            return Err(error);
        }
        state.requests.push(request);
        state.events.clear();
        state.ended = false;
        state.dropped = false;
        Ok((
            Box::new(Control(self.0.clone())),
            Box::new(Events(self.0.clone())),
        ))
    }
}
