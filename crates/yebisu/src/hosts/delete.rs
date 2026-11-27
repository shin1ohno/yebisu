//! Deleting a saved host (#250): every record this iPad keeps for it.
//!
//! The order makes a crash or a failed step safe. The records beside the
//! hosts file go first (the host-key pin, then the Herdr 2 layouts); the
//! hosts file is rewritten last, through the same pending file and rename as
//! a save, so it is either the old file or the new one. Until that rename the
//! host is still listed: a stop before it leaves a saved host whose pin may
//! be gone, which refuses to sign in (fail closed) until the user deletes it
//! again or trusts its key again. Deleting again finishes the job, because a
//! record that is already gone is not an error. After the rename only the
//! pre-list `yebisu-target` file can be left, and a hosts file shadows it.
//!
//! Never touched: the device key in the Keychain (one for the whole iPad),
//! and the iPad's public key in the host's `authorized_keys`, which this iPad
//! cannot reach. The launch record (`yebisu-connections.json`) drops a host
//! that is no longer saved by itself (`launch_restore.rs`).
use super::{SavedHosts, Store, parse};
use crate::{Error, Result};
use herdr_ssh::KnownHosts;
use std::{fs, path::Path};

impl Store {
    /// Forgets `target` and returns what is saved afterwards. `layouts` is
    /// the Herdr 2 layout directory (`core::workspace_root`). A target that
    /// is not saved changes nothing.
    pub(crate) fn delete(&self, target: &str, layouts: Option<&Path>) -> Result<SavedHosts> {
        let parsed = parse(target)?;
        let mut hosts = self.load_hosts()?;
        let Some(index) = hosts.targets.iter().position(|saved| saved == target) else {
            return Ok(hosts);
        };
        hosts.targets.remove(index);
        hosts.names.remove(target);
        if hosts.selected == target {
            hosts.selected = next_selection(&hosts.targets, index);
        }
        // Another saved target (another user, say) on the same host and port
        // still needs the pin: it is the same name in known_hosts.
        let name = parsed.known_hosts_name();
        let shared = hosts.targets.iter().any(|other| {
            parse(other).is_ok_and(|other| other.known_hosts_name().eq_ignore_ascii_case(&name))
        });
        if !shared {
            KnownHosts::new(self.root.join("ssh/known_hosts"))
                .forget(&parsed)
                .map_err(|error| Error::Ssh(error.to_string()))?;
        }
        crate::core::forget_host_layouts(layouts, target)?;
        self.write(&hosts)?;
        self.forget_legacy_target(target);
        Ok(hosts)
    }

    /// The single-host file of builds before the host list. The hosts file
    /// now exists and shadows it, so a failure here only leaves dead text.
    fn forget_legacy_target(&self, target: &str) {
        let path = self.root.join("yebisu-target");
        match fs::read_to_string(&path) {
            Ok(saved) if saved == target => {
                if let Err(error) = fs::remove_file(&path) {
                    tracing::warn!(%error, "legacy saved host not removed");
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(%error, "legacy saved host not read"),
        }
    }
}

/// The host that takes a deleted host's place in the list: the one after
/// it, or the one before when it was last; none when the list is empty.
pub(crate) fn next_selection(remaining: &[String], index: usize) -> String {
    remaining
        .get(index)
        .or_else(|| remaining.last())
        .cloned()
        .unwrap_or_default()
}
