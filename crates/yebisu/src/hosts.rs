//! Container-local endpoint persistence; host fingerprints are always read from pins.
use crate::{Error, Result};
use herdr_ssh::{KnownHosts, Pins, Target};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::PathBuf,
};

pub(crate) struct Store {
    pub(crate) root: PathBuf,
}

impl Store {
    pub(crate) fn load(&self) -> Result<String> {
        Ok(self.load_hosts()?.selected)
    }

    pub(crate) fn load_hosts(&self) -> Result<SavedHosts> {
        let path = self.root.join("yebisu-hosts.json");
        match fs::File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(65537).read_to_end(&mut bytes)?;
                SavedHosts::decode(&bytes)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let target = match fs::read_to_string(self.root.join("yebisu-target")) {
                    Ok(target) => {
                        parse(&target)?;
                        target
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(e) => return Err(e.into()),
                };
                Ok(SavedHosts {
                    targets: if target.is_empty() {
                        vec![]
                    } else {
                        vec![target.clone()]
                    },
                    selected: target,
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn save(&self, target: &str) -> Result<()> {
        parse(target)?;
        let mut hosts = self.load_hosts()?;
        if !hosts.targets.iter().any(|t| t == target) {
            if hosts.targets.len() >= 32 {
                return Err(invalid("at most 32 saved hosts"));
            }
            hosts.targets.push(target.to_owned());
        }
        hosts.selected = target.to_owned();
        fs::create_dir_all(&self.root)?;
        let path = self.root.join("yebisu-target.pending");
        let result = (|| -> Result<()> {
            let mut file = fs::File::create(&path)?;
            file.write_all(
                serde_json::json!({"version":1,"targets":hosts.targets,"selected":hosts.selected})
                    .to_string()
                    .as_bytes(),
            )?;
            file.sync_all()?;
            fs::rename(&path, self.root.join("yebisu-hosts.json"))?;
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

#[derive(Default)]
pub(crate) struct SavedHosts {
    pub targets: Vec<String>,
    pub selected: String,
}
fn invalid(message: &str) -> Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message).into()
}
impl SavedHosts {
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65536 {
            return Err(invalid("saved hosts too large"));
        }
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
        if value["version"] != 1 {
            return Err(invalid("unsupported saved hosts version"));
        }
        let targets = value["targets"]
            .as_array()
            .ok_or_else(|| invalid("missing hosts"))?;
        if targets.len() > 32 {
            return Err(invalid("too many hosts"));
        }
        let mut result = Self::default();
        for target in targets {
            let target = target.as_str().ok_or_else(|| invalid("invalid host"))?;
            parse(target)?;
            if result.targets.iter().any(|t| t == target) {
                return Err(invalid("duplicate host"));
            }
            result.targets.push(target.to_owned());
        }
        result.selected = value["selected"]
            .as_str()
            .ok_or_else(|| invalid("missing selection"))?
            .to_owned();
        if !result.targets.contains(&result.selected)
            && !(result.targets.is_empty() && result.selected.is_empty())
        {
            return Err(invalid("selection is not saved"));
        }
        Ok(result)
    }
}
