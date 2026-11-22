//! What the screen calls a saved host (MAIN-20, design fidelity plan
//! Wave 0a). The sidebar, the rail and the status bar show a short name; the
//! SSH destination stays the identity and the only thing that connects.
use herdr_ssh::Target;
use std::{fmt, net::Ipv4Addr};

/// A saved host's display name next to its SSH destination.
///
/// The destination (`user@host[:port]`) is what `Store` saves and what every
/// connection, host key pin and row identity keys on. The display name is
/// presentation only, with the destination supplying a fallback. Two destinations can share one display name (`me@pro-dev` and
/// `root@pro-dev:2222`), so a row that must tell them apart keeps the
/// destination too.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct HostName {
    destination: String,
    display: String,
}

impl HostName {
    /// The name of `destination`: the host's first DNS label, as the canvas
    /// shows `shin1ohno@pro-dev.home.local` as `pro-dev` (AddHost.dc.html:61,
    /// Main.dc.html:41), or the whole address for an IPv4 or IPv6 host. A
    /// string that is not an SSH target (a QA peer such as `qa-alpha`) shows
    /// as written; saved destinations always parse, as `Store` checks them.
    #[cfg(test)]
    pub(crate) fn new(destination: &str) -> Self {
        Self::with_saved(destination, None)
    }

    pub(crate) fn with_saved(destination: &str, saved: Option<&str>) -> Self {
        let display = match saved {
            Some(name) => name.to_owned(),
            None => match Target::parse(destination) {
                Ok(target) => short_host(&target.host).to_owned(),
                Err(_) => destination.to_owned(),
            },
        };
        Self {
            destination: destination.to_owned(),
            display,
        }
    }

    /// What SSH connects to, and the host's identity everywhere.
    pub(crate) fn destination(&self) -> &str {
        &self.destination
    }

    /// What the sidebar, the rail and the status bar draw.
    pub(crate) fn display(&self) -> &str {
        &self.display
    }
}

/// Formats as [`HostName::display`], the one place the drawn name comes from.
impl fmt::Display for HostName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display())
    }
}

/// The first label of a host name; an IP address stays whole. `Target`
/// stores IPv6 without brackets, so a `:` marks one.
fn short_host(host: &str) -> &str {
    if host.contains(':') || host.parse::<Ipv4Addr>().is_ok() {
        return host;
    }
    host.split('.')
        .next()
        .filter(|label| !label.is_empty())
        .unwrap_or(host)
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
