//! A recorded presenter in place of herdr-uikit: it keeps every request and
//! call, and the test pushes the session's events.
use crate::shell::search::{
    PresentError, Presented, SearchControl, SearchEvent, SearchEvents, SearchPresenter,
    SearchRequest, SearchResults,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[derive(Default)]
pub(crate) struct Recorded {
    pub(crate) requests: Vec<SearchRequest>,
    pub(crate) updates: Vec<(u64, SearchResults)>,
    pub(crate) queries: Vec<String>,
    pub(crate) dismissed: Vec<u64>,
    /// The control was dropped (the SDK handle released), and how many
    /// `dismiss_selected` calls it had seen by then.
    pub(crate) dropped_after: Option<usize>,
    pub(crate) refuse: Option<PresentError>,
    events: VecDeque<SearchEvent>,
    ended: bool,
    waker: Option<Waker>,
}

/// The test's handle on the fake: install it, push events, read calls.
#[derive(Clone, Default)]
pub(crate) struct RecordedSearch(pub(crate) Rc<RefCell<Recorded>>);

impl RecordedSearch {
    pub(crate) fn push(&self, event: SearchEvent) {
        let mut state = self.0.borrow_mut();
        if matches!(event, SearchEvent::InputRestored(_)) {
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

    pub(crate) fn request(&self) -> SearchRequest {
        self.0
            .borrow()
            .requests
            .last()
            .cloned()
            .unwrap_or_else(|| panic!("nothing was presented"))
    }

    /// The results on screen now: the last update, else the request's.
    pub(crate) fn shown(&self) -> (u64, SearchResults) {
        let state = self.0.borrow();
        state.updates.last().cloned().unwrap_or_else(|| {
            let request = state
                .requests
                .last()
                .unwrap_or_else(|| panic!("nothing was presented"));
            (0, request.results.clone())
        })
    }

    pub(crate) fn dropped(&self) -> bool {
        self.0.borrow().dropped_after.is_some()
    }
}

struct Control(Rc<RefCell<Recorded>>);

impl SearchControl for Control {
    fn update(&self, results: SearchResults) -> Result<u64, PresentError> {
        let mut state = self.0.borrow_mut();
        let revision = state.updates.len() as u64 + 1;
        state.updates.push((revision, results));
        Ok(revision)
    }
    fn dismiss_selected(&self, id: u64) -> Result<(), PresentError> {
        self.0.borrow_mut().dismissed.push(id);
        Ok(())
    }
    fn set_query(&self, query: &str) -> Result<(), PresentError> {
        self.0.borrow_mut().queries.push(query.to_owned());
        Ok(())
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        let dismissed = state.dismissed.len();
        state.dropped_after = Some(dismissed);
    }
}

struct Events(Rc<RefCell<Recorded>>);

impl SearchEvents for Events {
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<SearchEvent>> {
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

impl SearchPresenter for RecordedSearch {
    fn present(
        &mut self,
        _window: gpui::AnyWindowHandle,
        request: SearchRequest,
    ) -> Result<Presented, PresentError> {
        let mut state = self.0.borrow_mut();
        state.requests.push(request);
        if let Some(error) = state.refuse {
            return Err(error);
        }
        state.events.clear();
        state.ended = false;
        state.dropped_after = None;
        state.updates.clear();
        Ok((
            Box::new(Control(self.0.clone())),
            Box::new(Events(self.0.clone())),
        ))
    }
}
