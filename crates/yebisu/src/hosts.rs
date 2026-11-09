//! Container-local endpoint persistence; host fingerprints are always read from pins.
use crate::{Error, Result};
use herdr_ssh::{KnownHosts, Pins, Target};
use std::{fs, io::Write as _, path::PathBuf};

pub(crate) struct Store {
    pub(crate) root: PathBuf,
}

impl Store {
    pub(crate) fn load(&self) -> Result<String> {
        match fs::read_to_string(self.root.join("yebisu-target")) {
            Ok(target) => {
                parse(&target)?;
                Ok(target)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn save(&self, target: &str) -> Result<()> {
        parse(target)?;
        fs::create_dir_all(&self.root)?;
        let path = self.root.join("yebisu-target.pending");
        let result = (|| -> Result<()> {
            let mut file = fs::File::create(&path)?;
            file.write_all(target.as_bytes())?;
            file.sync_all()?;
            fs::rename(&path, self.root.join("yebisu-target"))?;
            fs::File::open(&self.root)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(path);
        }
        result
    }

    pub(crate) fn pins(&self, target: &str) -> Result<Pins> {
        KnownHosts::new(self.root.join("ssh/known_hosts"))
            .pins(&parse(target)?)
            .map_err(|e| Error::Ssh(e.to_string()))
    }

    pub(crate) fn fingerprints(&self, target: &str) -> Result<String> {
        match self.pins(target)? {
            Pins::None => Err(Error::UntrustedHost),
            Pins::Keys(keys) if keys.is_empty() => Err(Error::UntrustedHost),
            Pins::Keys(keys) => Ok(keys
                .iter()
                .map(|k| format!("{} {}", k.algorithm(), k.fingerprint()))
                .collect::<Vec<_>>()
                .join("\n")),
        }
    }
}

pub(crate) fn authentication_gate(scan: &sheprd_client::ssh::HostKeyScan) -> Result<()> {
    match scan {
        sheprd_client::ssh::HostKeyScan::Trusted => Ok(()),
        _ => Err(Error::UntrustedHost),
    }
}

fn parse(target: &str) -> Result<Target> {
    if target.len() > 1024 {
        return Err(Error::UntrustedHost);
    }
    Target::parse(target).map_err(|e| Error::Ssh(e.to_string()))
}

#[cfg(all(test, not(target_os = "ios")))]
mod tests;
