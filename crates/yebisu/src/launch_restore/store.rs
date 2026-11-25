//! `yebisu-connections.json` in the state directory, beside the saved hosts.
//! Read once at launch and written by one thread, newest record first, so
//! the UI thread never touches the disk.
use super::{Entry, Record};
use crate::shell::HerdrKind;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
};

const FILE: &str = "yebisu-connections.json";
const PENDING: &str = "yebisu-connections.pending";
const MAX_BYTES: usize = 65_536;
/// The saved-hosts limit (`hosts.rs`), once per Herdr.
const MAX_OTHERS: usize = 64;
const MAX_HOST_BYTES: usize = 1024;

fn invalid(message: &str) -> crate::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message).into()
}

fn kind_name(kind: HerdrKind) -> &'static str {
    match kind {
        HerdrKind::Herdr1 => "herdr1",
        HerdrKind::Herdr2 => "herdr2",
    }
}

fn entry_json(entry: &Entry) -> serde_json::Value {
    serde_json::json!({
        "host": entry.host,
        "kind": kind_name(entry.kind),
        "space": entry.space,
    })
}

pub(super) fn encode(record: &Record) -> Vec<u8> {
    serde_json::json!({
        "version": 1,
        "selected": record.selected.as_ref().map(entry_json),
        "others": record.others.iter().map(entry_json).collect::<Vec<_>>(),
    })
    .to_string()
    .into_bytes()
}

/// A space name the Herdr 2 connect command accepts (as `core::poll` checks
/// the names a host lists).
fn valid_space(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && !name.chars().any(char::is_control)
        && sheprd_client::core_connect_command(name).is_ok()
}

fn decode_entry(value: &serde_json::Value) -> crate::Result<Entry> {
    let host = value["host"]
        .as_str()
        .filter(|host| {
            !host.is_empty() && host.len() <= MAX_HOST_BYTES && !host.chars().any(char::is_control)
        })
        .ok_or_else(|| invalid("invalid host"))?;
    let kind = match value["kind"].as_str() {
        Some("herdr1") => HerdrKind::Herdr1,
        Some("herdr2") => HerdrKind::Herdr2,
        _ => return Err(invalid("invalid Herdr kind")),
    };
    let space = match &value["space"] {
        serde_json::Value::Null => None,
        serde_json::Value::String(space) if valid_space(space) => Some(space.clone()),
        _ => return Err(invalid("invalid space")),
    };
    if (kind == HerdrKind::Herdr2) != space.is_some() {
        return Err(invalid("a space belongs to Herdr 2 only"));
    }
    Ok(Entry {
        host: host.to_owned(),
        kind,
        space,
    })
}

pub(super) fn decode(bytes: &[u8]) -> crate::Result<Record> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid("launch record too large"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
    if value["version"] != 1 {
        return Err(invalid("unsupported launch record version"));
    }
    let selected = match &value["selected"] {
        serde_json::Value::Null => None,
        entry => Some(decode_entry(entry)?),
    };
    let others = value["others"]
        .as_array()
        .ok_or_else(|| invalid("missing others"))?;
    if others.len() > MAX_OTHERS {
        return Err(invalid("too many connections"));
    }
    Ok(Record {
        selected,
        others: others
            .iter()
            .map(decode_entry)
            .collect::<crate::Result<_>>()?,
    })
}

/// `None`: no record yet (first launch, or a build before this one).
pub(super) fn load(root: &Path) -> crate::Result<Option<Record>> {
    match fs::read(root.join(FILE)) {
        Ok(bytes) => decode(&bytes).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn save(root: &Path, record: &Record) -> crate::Result<()> {
    fs::create_dir_all(root)?;
    let pending = root.join(PENDING);
    let mut file = fs::File::create(&pending)?;
    file.write_all(&encode(record))?;
    file.sync_all()?;
    fs::rename(&pending, root.join(FILE))?;
    Ok(())
}

/// The one thread that writes the record; a burst of changes writes once.
pub(crate) struct Writer(mpsc::Sender<Record>);

impl Writer {
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(super) fn start(root: PathBuf) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Record>();
        std::thread::Builder::new()
            .name("yebisu-launch-record".into())
            .spawn(move || {
                while let Ok(mut record) = receiver.recv() {
                    while let Ok(newer) = receiver.try_recv() {
                        record = newer;
                    }
                    if let Err(error) = save(&root, &record) {
                        tracing::warn!(%error, "launch record not saved");
                    }
                }
            })?;
        Ok(Self(sender))
    }

    pub(super) fn send(&self, record: Record) {
        if self.0.send(record).is_err() {
            tracing::warn!("launch record writer stopped");
        }
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
