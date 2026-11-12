//! P15 read-only file snapshot model. No transport or host mutation capability.
//! Provenance and future adapter gates: docs/p15-readonly-model/README.md.
mod availability;
mod command;
#[cfg(test)]
mod command_tests;
mod parser;
#[cfg(test)]
mod tests;

pub(crate) use parser::{KnownSettings, parse};
use std::time::{Duration, Instant};

pub(crate) const MAX_BYTES: usize = 64 * 1024;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    UnknownPath,
    UnsafePath,
    TooLarge,
    InvalidUtf8,
    InvalidToml,
    InvalidField,
    PermissionDenied,
    Transport,
    Symlink,
    NotRegular,
    Timeout,
    Cancelled,
    Exhausted,
    InvalidHost,
    CommandTooLarge,
}

/// Data only: validation is not a filesystem/symlink or daemon provenance check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConfigPath(String);
impl ConfigPath {
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() {
            return Err(Error::UnknownPath);
        }
        if value.len() > 4096
            || !value.starts_with('/')
            || value == "/"
            || value
                .split('/')
                .skip(1)
                .any(|v| v.is_empty() || v == "." || v == "..")
            || !value
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || b"/_-. ".contains(&v))
        {
            return Err(Error::UnsafePath);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque saved-profile identity, never a display name or credential selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HostId(String);
impl HostId {
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(Error::InvalidHost);
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub host: HostId,
    pub connection_generation: u64,
    pub request_id: u64,
    pub path: ConfigPath,
}

/// The complete worker request vocabulary: only a bounded read, no arbitrary payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Request {
    Read { identity: Identity },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FileState {
    Missing,
    Parsed(KnownSettings),
    Failed(Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub identity: Identity,
    /// File values only. Never daemon-effective settings.
    pub file: FileState,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum State {
    #[default]
    Disconnected,
    Loading(Identity),
    Finished(Snapshot),
}

/// One outstanding request, no retained editable/old-host snapshot or retry loop.
#[derive(Default)]
pub(crate) struct Model {
    state: State,
    deadline: Option<Instant>,
    next_request: u64,
}
impl Model {
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Replaces any previous read. The adapter must cancel its old channel first.
    /// Keep this model for the entire view lifetime so IDs cannot be reused.
    pub fn begin(
        &mut self,
        host: HostId,
        connection_generation: u64,
        path: ConfigPath,
        now: Instant,
    ) -> Result<Request, Error> {
        self.disconnect();
        let request_id = self.next_request.checked_add(1).ok_or(Error::Exhausted)?;
        let deadline = now.checked_add(TIMEOUT).ok_or(Error::Exhausted)?;
        self.next_request = request_id;
        let identity = Identity {
            host,
            connection_generation,
            request_id,
            path,
        };
        self.state = State::Loading(identity.clone());
        self.deadline = Some(deadline);
        Ok(Request::Read { identity })
    }

    /// The caller supplies monotonic time; a late packet cannot beat expiration.
    pub fn complete(&mut self, snapshot: Snapshot, now: Instant) -> bool {
        self.expire(now);
        if !matches!(&self.state, State::Loading(id) if id == &snapshot.identity) {
            return false;
        }
        self.deadline = None;
        self.state = State::Finished(snapshot);
        true
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.finish_error(Error::Timeout);
            return true;
        }
        false
    }

    pub fn cancel(&mut self) {
        self.finish_error(Error::Cancelled);
    }
    pub fn disconnect(&mut self) {
        self.deadline = None;
        self.state = State::Disconnected;
    }
    fn finish_error(&mut self, error: Error) {
        if let State::Loading(identity) = &self.state {
            self.state = State::Finished(Snapshot {
                identity: identity.clone(),
                file: FileState::Failed(error),
            });
        }
        self.deadline = None;
    }
}
