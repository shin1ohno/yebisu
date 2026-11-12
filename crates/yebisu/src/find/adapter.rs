//! Reader-shaped mailbox: at most one registered request and one typed answer.
use super::{Scope, State};
use crate::{live::Live, terminal::InputTarget};
use sheprd_client::{Method, scrollback::CopySearchResult};

#[derive(Clone, Default)]
pub(crate) struct Mailbox {
    pub ticket: Option<(Scope, String)>,
    pub answer: Option<Result<CopySearchResult, State>>,
}
impl Mailbox {
    pub fn clear(&mut self) {
        self.ticket = None;
        self.answer = None;
    }
}

pub(crate) fn scope(live: &Live, host: &str, generation: u64) -> Option<Scope> {
    let destination = crate::terminal_input::destination(live, generation)?;
    let InputTarget::Pane(pane) = destination.target else {
        return None;
    };
    Some(Scope {
        host: host.to_owned(),
        generation,
        epoch: destination.epoch,
        boot: destination.boot,
        pane,
    })
}
pub(crate) fn capable(live: &Live) -> bool {
    Method::PaneCopySearch.advertised_in(&live.methods)
}

/// Called by Live's only reader BEFORE a response can be coalesced away.
/// Connection identity is implicit in the Live instance, never a global ID map.
pub(crate) fn receive(live: &mut Live, id: &str, response: &serde_json::Value) {
    let Some((expected, request)) = live.search.ticket.as_ref() else {
        return;
    };
    if request != id || live.search.answer.is_some() {
        return;
    }
    if scope(live, &expected.host, expected.generation).as_ref() != Some(expected) {
        live.search.clear();
        return;
    }
    live.search.answer = Some(super::decode(response));
}
