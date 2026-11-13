//! All Keychain, network and filesystem work lives in background jobs.
use crate::Result;
#[cfg(target_os = "ios")]
use crate::{
    Error,
    hosts::{Store, authentication_gate},
};
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
}

#[cfg(any(target_os = "ios", test))]
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(crate) enum Job {
    Load,
    Save(String),
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
    Saved(crate::hosts::SavedHosts, String),
    Authenticated(crate::hosts::SavedHosts, String),
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
            #[cfg(target_abi = "sim")]
            if std::env::var_os("YEBISU_P5_QA").is_some() {
                store.save("qa@private-one")?;
                store.save("qa@private-two")?;
            }
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
        Job::Save(target) => {
            let store = store()?;
            store.save(&target)?;
            Ok(Reply::Hosts(store.load_hosts()?, String::new()))
        }
        Job::Scan(target) => {
            let scan = ssh::scan_host_key(&target).map_err(map)?;
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
            let store = store()?;
            store.save(&target)?;
            Ok(Reply::Authenticated(
                store.load_hosts()?,
                format!("SSH authenticated: {probe:?}"),
            ))
        }
    }
}

impl Setup {
    pub(crate) fn apply(&mut self, reply: Result<Reply>) {
        self.busy = false;
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
            Ok(Reply::Saved(hosts, fingerprints)) => {
                self.target = hosts.selected;
                self.hosts = hosts.targets;
                self.pending = None;
                self.fingerprints = fingerprints;
                self.status = "Host key trusted and connection saved.".into();
            }
            Ok(Reply::Authenticated(hosts, message)) => {
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
