//! Connection-scoped terminal search. Modeled on the pinned fork's find.rs.
//! The caller serializes send/registration with the connection reader mailbox.
pub(crate) mod adapter;
pub(crate) mod field;
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "p13-native-probe"))]
mod native_probe;
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "p13-native-probe"))]
mod native_queries;
pub(crate) mod retirement;
use sheprd_client::scrollback::{
    CopySearchParams, CopySearchResult, MAX_SEARCH_QUERY_BYTES, SearchDirection, TextPoint,
};
use std::time::{Duration, Instant};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const MAX_MATCHES: usize = 512;
pub(crate) const MAX_RESPONSE_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scope {
    pub host: String,
    pub generation: u64,
    pub epoch: u64,
    pub boot: String,
    pub pane: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Empty,
    Waiting,
    Matches,
    NoMatch,
    Unsupported,
    SendFailed,
    TimedOut,
    Malformed,
    Oversized,
    Closed,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "Search terminal",
            Self::Waiting => "Searching…",
            Self::Matches => "Match found",
            Self::NoMatch => "No matches",
            Self::Unsupported => "This host does not advertise terminal search",
            Self::SendFailed => "Search could not be sent",
            Self::TimedOut => "Search timed out; edit query to retry",
            Self::Malformed => "Host returned an invalid search response",
            Self::Oversized => "Search exceeds the supported size",
            Self::Closed => "Search closed: target changed",
        }
    }
}
struct Flight {
    id: String,
    query: String,
    revision: u64,
    sent: Instant,
}

pub(crate) struct Search {
    pub scope: Scope,
    pub state: State,
    pub query: String,
    pub result: Option<CopySearchResult>,
    flight: Option<Flight>,
    queued: Option<SearchDirection>,
}
impl Search {
    pub fn new(scope: Scope, supported: bool) -> Self {
        Self {
            scope,
            state: if supported {
                State::Empty
            } else {
                State::Unsupported
            },
            query: String::new(),
            result: None,
            flight: None,
            queued: None,
        }
    }
    pub fn set_query(&mut self, query: &str) {
        if matches!(self.state, State::Closed | State::Unsupported) {
            return;
        }
        if self.query == query
            && !matches!(
                self.state,
                State::TimedOut | State::SendFailed | State::Malformed | State::Oversized
            )
        {
            return;
        }
        self.result = None;
        self.queued = None;
        self.query.clear();
        if query.len() > MAX_SEARCH_QUERY_BYTES {
            self.state = State::Oversized;
        } else {
            self.query.push_str(query);
            self.state = if query.is_empty() {
                State::Empty
            } else {
                State::Waiting
            };
            if !query.is_empty() {
                self.queued = Some(SearchDirection::Backward);
            }
        }
    }
    pub fn step(&mut self, direction: SearchDirection) {
        if self.query.is_empty()
            || matches!(
                self.state,
                State::Closed | State::Unsupported | State::Oversized
            )
        {
            return;
        }
        self.queued = Some(direction);
        self.state = State::Waiting;
    }
    pub fn next(&self, revision: u64, cursor: TextPoint) -> Option<CopySearchParams> {
        if self.flight.is_some() || self.query.is_empty() || !revision.is_multiple_of(2) {
            return None;
        }
        Some(CopySearchParams {
            pane_id: self.scope.pane.clone(),
            query: self.query.clone(),
            direction: self.queued?,
            cursor,
            content_revision: revision,
            previous: self
                .result
                .as_ref()
                .filter(|r| r.content_revision == revision)
                .and_then(CopySearchResult::current_match),
        })
    }
    pub fn sent(&mut self, id: String, params: &CopySearchParams, now: Instant) {
        self.queued = None;
        self.flight = Some(Flight {
            id,
            query: params.query.clone(),
            revision: params.content_revision,
            sent: now,
        });
    }
    pub fn request_id(&self) -> Option<&str> {
        self.flight.as_ref().map(|f| f.id.as_str())
    }
    pub fn send_failed(&mut self) {
        self.queued = None;
        self.state = State::SendFailed;
    }
    pub fn close(&mut self) {
        self.flight = None;
        self.queued = None;
        self.result = None;
        self.query.clear();
        self.state = State::Closed;
    }
    /// Timeout never replays a flight, even if the text changed while waiting.
    pub fn expire(&mut self, now: Instant) -> bool {
        if !self
            .flight
            .as_ref()
            .is_some_and(|f| now.saturating_duration_since(f.sent) >= TIMEOUT)
        {
            return false;
        }
        self.flight = None;
        self.queued = None;
        self.result = None;
        self.state = State::TimedOut;
        true
    }
    pub fn answer(
        &mut self,
        scope: &Scope,
        id: &str,
        answer: Result<CopySearchResult, State>,
    ) -> bool {
        if scope != &self.scope || self.request_id() != Some(id) {
            return false;
        }
        let Some(flight) = self.flight.take() else {
            return false;
        };
        if flight.query != self.query || self.query.is_empty() {
            return false;
        }
        match answer {
            Err(state) => {
                self.state = state;
                self.result = None;
            }
            Ok(result) => {
                if result.pane_id != self.scope.pane || result.content_revision != flight.revision {
                    self.state = State::Malformed;
                    self.result = None;
                    return false;
                }
                self.state = if result.total == 0 {
                    State::NoMatch
                } else {
                    State::Matches
                };
                self.result = Some(result);
            }
        }
        true
    }
}

/// Validate before decoding allocates a typed range vector. The SDK already
/// owns the JSON Value, so this bounds retained consumer state, not transport.
pub(crate) fn decode(value: &serde_json::Value) -> Result<CopySearchResult, State> {
    let ranges = value
        .pointer("/result/matches")
        .and_then(serde_json::Value::as_array);
    if ranges.is_some_and(|m| m.len() > MAX_MATCHES) {
        return Err(State::Oversized);
    }
    // Serialize into a capped writer rather than allocating a second full body.
    struct Cap(usize);
    impl std::io::Write for Cap {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            if self.0 > MAX_RESPONSE_BYTES {
                return Err(std::io::ErrorKind::FileTooLarge.into());
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Cap(0), value).map_err(|_| State::Oversized)?;
    let result =
        sheprd_client::scrollback::decode_copy_search(value).map_err(|_| State::Malformed)?;
    let empty = result.total == 0;
    if result.matches.len() as u64 > result.total
        || result.matches.iter().any(|r| r.start > r.end)
        || (empty
            && (!result.matches.is_empty()
                || result.current.is_some()
                || result.current_global.is_some()))
        || (!empty
            && (result.current_match().is_none()
                || result.current_global.is_none_or(|i| i >= result.total)))
    {
        return Err(State::Malformed);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../find_tests.rs"]
mod tests;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
pub(crate) mod peer;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) mod qa;
