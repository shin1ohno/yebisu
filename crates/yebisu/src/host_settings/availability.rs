//! Current production provider has no retained read authority. Fail before exec.
//! This module imports neither SSH nor Keychain and creates no worker or retry.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Availability {
    Disconnected,
    CredentialUnavailable,
}

/// Evaluate the live selected connection at render time, not an opening snapshot.
pub(crate) fn current(connected: bool) -> Availability {
    if connected {
        Availability::CredentialUnavailable
    } else {
        Availability::Disconnected
    }
}

impl Availability {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected — host settings unavailable",
            Self::CredentialUnavailable => {
                "Unavailable — this connection cannot authorize a read-only settings request"
            }
        }
    }
}

#[cfg(test)]
#[path = "availability_tests.rs"]
mod tests;
