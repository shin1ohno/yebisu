//! Why a forward was refused, could not start, or ended, worded for the
//! status line and the forward's row. Every text is built from a typed
//! variant: nothing a host wrote (an error's description, a banner, command
//! output) reaches the screen, `qa.state` or a log line, because the host's
//! text is untrusted data (AGENTS.md).
use herdr_ssh::{Failure, forward};

/// What the forward sheet refuses before anything connects; the texts are
/// Sheprd's (`error-port-forward.ftl`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum InputError {
    // keep-sorted start
    #[error("Port {0} is already forwarded from this host.")]
    Duplicate(u16),
    #[error("At most {0} ports can be forwarded at once.")]
    Limit(usize),
    #[error("Enter a port number from 1 to 65535.")]
    Port,
    // keep-sorted end
}

/// Why a forward ended, shown on its row's second line and in the status
/// line. A forward the user stops has no reason: its row goes away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    // keep-sorted start
    /// The connection came up and then went away.
    ConnectionLost,
    /// The connection did not come up: a timeout, an unreachable host, a
    /// protocol failure, or a local failure before the first packet.
    ConnectTimeout,
    /// The host refused this iPad's key.
    KeyRefused,
    /// No loopback port on this iPad could be listened on.
    NoLocalPort,
    /// The host does not allow this key to forward (`restrict`,
    /// `permitopen`, `AllowTcpForwarding no`, `DisableForwarding yes`).
    Prohibited,
    /// The host refused the channel for another reason, or the forward
    /// already carries as many connections as it can.
    Refused,
    /// The host has no pinned key, or its key changed.
    Untrusted,
    // keep-sorted end
}

impl Reason {
    /// The reason a forward's error gives. `None` for [`forward::Error::Stopped`]
    /// (the app stopped it) and [`forward::Error::PortClosed`] (the forward
    /// listens; nothing on the host accepts on that port yet).
    pub(crate) fn from_forward(error: &forward::Error) -> Option<Self> {
        Some(match error {
            forward::Error::Prohibited => Self::Prohibited,
            forward::Error::Connect(failure) => Self::from_connect(*failure),
            forward::Error::ConnectionLost(_) => Self::ConnectionLost,
            forward::Error::Io(_) | forward::Error::Timeout => Self::ConnectTimeout,
            forward::Error::Busy
            | forward::Error::InvalidPort
            | forward::Error::NotLoopback
            | forward::Error::Refused => Self::Refused,
            forward::Error::PortClosed | forward::Error::Stopped => return None,
        })
    }

    /// The reason a connection that never came up gives.
    pub(crate) fn from_connect(failure: Failure) -> Self {
        match failure {
            Failure::Auth => Self::KeyRefused,
            Failure::HostKeyChanged | Failure::HostKeyUnknown => Self::Untrusted,
            Failure::Cancelled | Failure::Protocol | Failure::Timeout | Failure::Unreachable => {
                Self::ConnectTimeout
            }
        }
    }

    /// The reason the host-key gate before connecting gives: a refusal is
    /// [`Reason::Untrusted`]; a scan that could not reach the host is a
    /// connection that did not come up.
    pub(crate) fn from_gate(error: &crate::Error) -> Self {
        match error {
            crate::Error::UntrustedHost => Self::Untrusted,
            _ => Self::ConnectTimeout,
        }
    }

    /// The reason `sheprd_client::ssh::forward_connect` gives when it
    /// refuses before any connection starts.
    #[cfg(target_os = "ios")]
    pub(crate) fn from_client(error: &sheprd_client::Error) -> Self {
        use sheprd_client::ssh::{Error as Ssh, SshFailure, in_process};
        match error {
            sheprd_client::Error::Ssh(Ssh::InProcess(in_process::Error::Ssh(
                herdr_ssh::Error::Forward(error),
            ))) => Self::from_forward(error).unwrap_or(Self::ConnectTimeout),
            sheprd_client::Error::Ssh(Ssh::Refused(SshFailure::HostKey)) => Self::Untrusted,
            sheprd_client::Error::Ssh(Ssh::Refused(SshFailure::Auth)) => Self::KeyRefused,
            _ => Self::ConnectTimeout,
        }
    }

    /// The words after `forward ended: `. `host` is the host's display
    /// name, `remote` the forwarded port.
    pub(crate) fn text(self, host: &str, remote: u16) -> String {
        match self {
            Self::Prohibited => format!(
                "{host} does not allow port forwarding for this iPad's key. Remove restrict from this iPad's line in ~/.ssh/authorized_keys on {host}."
            ),
            Self::ConnectTimeout => {
                "SSH did not connect in time; check host trust and authentication".into()
            }
            Self::KeyRefused => format!("{host} did not accept this iPad's key"),
            Self::Untrusted => crate::Error::UntrustedHost.to_string(),
            Self::ConnectionLost => format!("SSH connection to {host} ended; check the network"),
            Self::NoLocalPort => "no local port is free to forward to".into(),
            Self::Refused => format!("{host} refused to forward port {remote}"),
        }
    }

    /// The reason's name in logs and `qa.state`: a fixed word, never text.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ConnectTimeout => "connect_timeout",
            Self::ConnectionLost => "connection_lost",
            Self::KeyRefused => "key_refused",
            Self::NoLocalPort => "no_local_port",
            Self::Prohibited => "prohibited",
            Self::Refused => "refused",
            Self::Untrusted => "untrusted",
        }
    }
}
