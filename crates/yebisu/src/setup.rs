//! All Keychain, network and filesystem work lives in background jobs.
use crate::Result;
#[cfg(target_os = "ios")]
use crate::{
    Error,
    hosts::{Store, authentication_gate},
    keys,
};
#[cfg(target_os = "ios")]
use sheprd_client::ssh;
use sheprd_client::ssh::{HostKeyScan, NewHostKey};

#[derive(Default)]
pub(crate) struct Setup {
    pub(crate) busy: bool,
    pub(crate) status: String,
    pub(crate) target: String,
    pub(crate) key_line: Option<String>,
    pub(crate) fingerprints: String,
    pub(crate) pending: Option<(String, NewHostKey)>,
}

#[cfg(target_os = "ios")]
pub(crate) enum Job {
    Load,
    Key,
    Scan(String),
    Trust(String, NewHostKey),
    Authenticate(String),
}

pub(crate) enum Reply {
    Loaded(String, String),
    Key(String),
    Scanned(String, HostKeyScan, String),
    Saved(String, String),
    Authenticated(String),
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
            Ok(Reply::Loaded(target, fingerprints))
        }
        Job::Key => {
            let raw = ssh::authorized_key().map_err(map)?;
            // No other path may display or copy the raw dependency comment.
            Ok(Reply::Key(keys::authorized_line(&raw)?))
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
            Ok(Reply::Saved(target.clone(), store.fingerprints(&target)?))
        }
        Job::Authenticate(target) => {
            // Scan sends no credentials. The authenticated probe independently checks
            // pins on its new connection, closing the scan/connect race.
            authentication_gate(&ssh::scan_host_key(&target).map_err(map)?)?;
            let probe = ssh::probe_host(&target, "default").map_err(map)?;
            if let ssh::HostProbe::SshFailed(failure) = probe {
                return Err(Error::Ssh(format!("authentication refused: {failure:?}")));
            }
            store()?.save(&target)?;
            Ok(Reply::Authenticated(format!(
                "SSH authenticated: {probe:?}"
            )))
        }
    }
}

impl Setup {
    pub(crate) fn apply(&mut self, reply: Result<Reply>) {
        self.busy = false;
        match reply {
            Err(e) => {
                self.status = e.to_string();
                self.pending = None;
            }
            Ok(Reply::Loaded(target, fingerprints)) => {
                self.target = target;
                self.fingerprints = fingerprints;
                self.status = "Enter user@host[:port], then scan the host key.".into();
            }
            Ok(Reply::Key(line)) => {
                self.key_line = Some(line);
                self.status =
                    "Copy this restricted public key to the host's authorized_keys.".into();
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
            Ok(Reply::Saved(target, fingerprints)) => {
                self.target = target;
                self.pending = None;
                self.fingerprints = fingerprints;
                self.status = "Host key trusted and connection saved.".into();
            }
            Ok(Reply::Authenticated(message)) => {
                self.status = message;
            }
        }
    }
}

// Deliberately no credential regeneration or host-key forgetting in P2.

#[cfg(test)]
mod tests;
