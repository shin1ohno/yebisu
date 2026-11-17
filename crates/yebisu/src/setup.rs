//! All Keychain, network and filesystem work lives in background jobs.
#[cfg(target_os = "ios")]
use crate::{
    Error,
    hosts::{Store, authentication_gate},
};
use crate::{Result, shell::add_host::Probe};
#[cfg(target_os = "ios")]
use sheprd_client::ssh;
use sheprd_client::ssh::{HostKeyScan, NewHostKey};

#[derive(Default)]
pub(crate) struct Setup {
    pub(crate) busy: bool,
    pub(crate) hosts: Vec<String>,
    pub(crate) status: String,
    pub(crate) target: String,
    pub(crate) fingerprints: String,
    pub(crate) pending: Option<(String, NewHostKey)>,
    /// Bumped by Cancel; a reply that carries an older value is dropped.
    pub(crate) epoch: u64,
    /// The job in flight is the startup Load.
    pub(crate) loading: bool,
    /// The startup Load answered (with hosts or with an error).
    pub(crate) loaded: bool,
}

#[cfg(any(target_os = "ios", test))]
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(crate) enum Job {
    Load,
    SaveFailure,
    Scan(String),
    Trust(String, NewHostKey),
    Authenticate(String),
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(crate) enum Reply {
    Loaded(String, String),
    Hosts(crate::hosts::SavedHosts, String),
    Scanned(String, HostKeyScan, String),
    /// The host offered a key that differs from the pinned one: both
    /// fingerprints, nothing trusted.
    Mismatch {
        target: String,
        expected: String,
        seen: String,
    },
    Saved(crate::hosts::SavedHosts, String),
    // Only tests still send it (setup/tests.rs, live/tests.rs): the L7 sign-in
    // replies SignedIn. Drop it with those tests' move to SignedIn.
    #[cfg_attr(target_os = "ios", allow(dead_code))]
    Authenticated(crate::hosts::SavedHosts, String),
    /// Sign-in succeeded; the probe says which Herdr the host runs.
    SignedIn(crate::hosts::SavedHosts, String, Probe),
}

#[cfg(target_os = "ios")]
fn probe_facts(probe: &ssh::HostProbe) -> Probe {
    use ssh::HostProbe;
    let (herdr1, herdr2, outdated) = match probe {
        HostProbe::Running | HostProbe::Stopped => (true, false, false),
        HostProbe::CoreAvailable => (false, true, false),
        HostProbe::OutdatedCoreAvailable => (false, true, true),
        HostProbe::Outdated => (false, false, true),
        HostProbe::Missing | HostProbe::SshFailed(_) => (false, false, false),
    };
    Probe {
        herdr1,
        herdr2,
        outdated,
    }
}

#[cfg(target_os = "ios")]
fn join_fingerprints(list: &[ssh::HostKeyFingerprint]) -> String {
    list.iter()
        .map(|f| format!("{} {}", f.kind, f.fingerprint))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(target_os = "ios")]
fn store() -> Result<Store> {
    let root = sheprd_client::local_path::state_dir()
        .ok_or_else(|| Error::Ssh("missing application container".into()))?;
    Ok(Store { root })
}

#[cfg(target_os = "ios")]
pub(crate) fn execute(job: Job) -> Result<Reply> {
    let map = |e: sheprd_client::Error| Error::Ssh(e.to_string());
    match job {
        Job::Load => {
            let store = store()?;
            let target = store.load()?;
            let fingerprints = if target.is_empty() {
                String::new()
            } else {
                match store.fingerprints(&target) {
                    Ok(value) => value,
                    Err(Error::UntrustedHost) => "Host has no trusted key; scan again.".into(),
                    Err(e) => return Err(e),
                }
            };
            Ok(Reply::Hosts(store.load_hosts()?, fingerprints))
        }
        Job::SaveFailure => Err(Error::Storage(std::io::Error::other(
            "Developer-injected save failure; saved hosts unchanged",
        ))),
        Job::Scan(target) => {
            let scan = ssh::scan_host_key(&target).map_err(map)?;
            if matches!(scan, HostKeyScan::Refused(ssh::SshFailure::HostKey))
                && let Ok(Some(change)) = ssh::changed_host_key(&target)
            {
                return Ok(Reply::Mismatch {
                    expected: join_fingerprints(&change.pinned),
                    seen: join_fingerprints(&change.offered.fingerprints),
                    target,
                });
            }
            let fingerprints = match &scan {
                HostKeyScan::New(key) => key
                    .fingerprints
                    .iter()
                    .map(|f| format!("{} {}", f.kind, f.fingerprint))
                    .collect::<Vec<_>>()
                    .join("\n"),
                HostKeyScan::Trusted => store()?.fingerprints(&target)?,
                HostKeyScan::Refused(_) => String::new(),
            };
            Ok(Reply::Scanned(target, scan, fingerprints))
        }
        Job::Trust(target, key) => {
            ssh::trust_host_key(&key).map_err(map)?;
            let store = store()?;
            // Failure is displayed even when pins were saved but the target was not.
            store.save(&target)?;
            Ok(Reply::Saved(
                store.load_hosts()?,
                store.fingerprints(&target)?,
            ))
        }
        Job::Authenticate(target) => {
            // Scan sends no credentials. The authenticated probe independently checks
            // pins on its new connection, closing the scan/connect race.
            authentication_gate(&ssh::scan_host_key(&target).map_err(map)?)?;
            let probe = ssh::probe_host(&target, "default").map_err(map)?;
            if let ssh::HostProbe::SshFailed(failure) = probe {
                return Err(Error::Ssh(format!("authentication refused: {failure:?}")));
            }
            let facts = probe_facts(&probe);
            let store = store()?;
            store.save(&target)?;
            Ok(Reply::SignedIn(
                store.load_hosts()?,
                format!("SSH authenticated: {probe:?}"),
                facts,
            ))
        }
    }
}

impl Setup {
    pub(crate) fn apply(&mut self, reply: Result<Reply>) {
        self.busy = false;
        if std::mem::take(&mut self.loading) {
            self.loaded = true;
        }
        match reply {
            Err(e) => {
                crate::diagnostics::record("setup_error");
                self.status = e.to_string();
                self.pending = None;
            }
            Ok(Reply::Loaded(target, fingerprints)) => {
                self.target = target;
                self.fingerprints = fingerprints;
                self.status = "Enter user@host[:port], then scan the host key.".into();
            }
            Ok(Reply::Hosts(hosts, fingerprints)) => {
                self.apply(Ok(Reply::Loaded(hosts.selected, fingerprints)));
                self.hosts = hosts.targets;
                self.status = "Saved hosts loaded. Select a host or enter a new target.".into();
            }
            Ok(Reply::Scanned(target, scan, fingerprints)) => {
                self.target = target.clone();
                self.fingerprints = fingerprints;
                self.pending = None;
                match scan {
                    HostKeyScan::New(key) => {
                        self.pending = Some((target, key));
                        self.status =
                            "Compare this fingerprint on the host before tapping Trust.".into();
                    }
                    HostKeyScan::Trusted => {
                        self.status = "Pinned host key verified.".into();
                    }
                    HostKeyScan::Refused(failure) => {
                        self.status = format!(
                            "Host refused: {failure:?}. No key trusted or credentials sent."
                        );
                    }
                }
            }
            Ok(Reply::Mismatch { target, .. }) => {
                self.target = target;
                self.fingerprints.clear();
                self.pending = None;
                self.status = "Host key changed. Refused; nothing was trusted.".into();
            }
            Ok(Reply::Saved(hosts, fingerprints)) => {
                self.target = hosts.selected;
                self.hosts = hosts.targets;
                self.pending = None;
                self.fingerprints = fingerprints;
                self.status = "Host key trusted and connection saved.".into();
            }
            Ok(Reply::Authenticated(hosts, message) | Reply::SignedIn(hosts, message, _)) => {
                self.target = hosts.selected;
                self.hosts = hosts.targets;
                self.status = message;
            }
        }
    }
}

// Deliberately no credential regeneration or host-key forgetting in P2.

#[cfg(test)]
mod tests;

// Host UI adapter tests never access a real credential store or host.
#[cfg(all(test, not(target_os = "ios")))]
pub(crate) fn execute(_: Job) -> Result<Reply> {
    Err(crate::Error::Ssh(
        "native setup is unavailable in host UI tests".into(),
    ))
}
