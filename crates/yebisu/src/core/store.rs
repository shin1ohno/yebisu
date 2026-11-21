//! Persist tab layouts per host and named Herdr 2 session.
use sheprd_backend::TerminalId;
use std::{
    collections::HashSet,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

pub(super) static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const MAX_STATE_BYTES: usize = 65_536;

#[derive(Clone, Default)]
pub(super) struct Saved {
    pub ids: Vec<TerminalId>,
    pub active: usize,
    pub vertical: bool,
}

pub(super) struct Tab {
    pub id: String,
    pub label: String,
    pub layout: Saved,
}

#[derive(Default)]
pub(super) struct Workspace {
    pub created: u64,
    pub tabs: Vec<Tab>,
    pub active: usize,
}

fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn prefix(host: &str) -> String {
    format!("core-{}-", hex(host))
}

fn path(root: &Path, host: &str, session: &str) -> PathBuf {
    let suffix = if session == "default" {
        "default".to_owned()
    } else {
        hex(session)
    };
    root.join(format!("{}{suffix}.json", prefix(host)))
}

pub(super) fn sessions(root: Option<&Path>, host: &str) -> crate::Result<Vec<String>> {
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    let files = match std::fs::read_dir(root) {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let prefix = prefix(host);
    let mut entries = Vec::new();
    for file in files {
        let file = file?;
        let name = file.file_name();
        let Some(suffix) = name
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        let session = if suffix == "default" {
            Some("default".to_owned())
        } else {
            decode_hex(suffix)
        };
        if let Some(session) = session.filter(|name| !name.is_empty()) {
            entries.push(session);
        }
    }
    entries.sort();
    Ok(entries)
}

fn decode_hex(suffix: &str) -> Option<String> {
    if !suffix.is_ascii() || !suffix.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..suffix.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&suffix[offset..offset + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

fn layout(value: &serde_json::Value) -> crate::Result<Saved> {
    let ids = value["ids"].as_array().ok_or(crate::Error::CoreState)?;
    if ids.len() > 2 {
        return Err(crate::Error::CoreState);
    }
    let ids = ids
        .iter()
        .map(|id| {
            TerminalId::new(id.as_str().ok_or(crate::Error::CoreState)?).map_err(crate::Error::from)
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let active = index(value, ids.len())?;
    Ok(Saved {
        ids,
        active,
        vertical: value["vertical"].as_bool().ok_or(crate::Error::CoreState)?,
    })
}

pub(super) fn milliseconds(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

fn index(value: &serde_json::Value, count: usize) -> crate::Result<usize> {
    let active = value["active"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(crate::Error::CoreState)?;
    if active >= count.max(1) {
        return Err(crate::Error::CoreState);
    }
    Ok(active)
}

pub(super) fn load_workspace(
    root: Option<&Path>,
    host: &str,
    session: &str,
) -> crate::Result<Workspace> {
    let Some(root) = root else {
        return Ok(Workspace::default());
    };
    let file = match std::fs::File::open(path(root, host, session)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Workspace::default());
        }
        Err(error) => return Err(error.into()),
    };
    let legacy_created = file.metadata()?.created().ok().map_or(0, milliseconds);
    let mut bytes = Vec::new();
    file.take((MAX_STATE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err(crate::Error::CoreState);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| crate::Error::CoreState)?;
    if value["host"] != host || value["session"] != session {
        return Err(crate::Error::CoreState);
    }
    if value["version"] == 1 && session == "default" {
        let layout = layout(&value)?;
        let tabs = layout.ids.first().map(|id| Tab {
            id: id.as_str().to_owned(),
            label: String::new(),
            layout: layout.clone(),
        });
        return Ok(Workspace {
            created: legacy_created,
            tabs: tabs.into_iter().collect(),
            active: 0,
        });
    }
    if value["version"] != 2 {
        return Err(crate::Error::CoreState);
    }
    let mut tab_ids = HashSet::new();
    let mut panes = HashSet::new();
    let tabs = value["tabs"]
        .as_array()
        .ok_or(crate::Error::CoreState)?
        .iter()
        .map(|tab| {
            let id = tab["id"].as_str().ok_or(crate::Error::CoreState)?;
            let label = tab["label"].as_str().ok_or(crate::Error::CoreState)?;
            let layout = layout(tab)?;
            if id.is_empty()
                || !tab_ids.insert(id.to_owned())
                || layout.ids.is_empty()
                || !layout.ids.iter().all(|id| panes.insert(id.clone()))
            {
                return Err(crate::Error::CoreState);
            }
            Ok(Tab {
                id: id.to_owned(),
                label: label.to_owned(),
                layout,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let active = index(&value, tabs.len())?;
    Ok(Workspace {
        created: value["created"].as_u64().unwrap_or(legacy_created),
        tabs,
        active,
    })
}

pub(super) fn save_workspace(
    root: Option<&Path>,
    host: &str,
    session: &str,
    workspace: &Workspace,
) -> crate::Result<()> {
    let tabs = workspace.tabs.iter().map(|tab| {
        serde_json::json!({
            "id": tab.id, "label": tab.label,
            "ids": tab.layout.ids.iter().map(TerminalId::as_str).collect::<Vec<_>>(),
            "active": tab.layout.active, "vertical": tab.layout.vertical
        })
    });
    save_value(
        root,
        host,
        session,
        serde_json::json!({
            "version": 2, "host": host, "session": session, "created": workspace.created,
            "tabs": tabs.collect::<Vec<_>>(), "active": workspace.active
        }),
    )
}

fn save_value(
    root: Option<&Path>,
    host: &str,
    session: &str,
    value: serde_json::Value,
) -> crate::Result<()> {
    let Some(root) = root else {
        return Ok(());
    };
    let bytes = serde_json::to_vec(&value).map_err(|_| crate::Error::CoreState)?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err(crate::Error::CoreState);
    }
    std::fs::create_dir_all(root)?;
    let path = path(root, host, session);
    let pending = path.with_extension("pending");
    let result = (|| {
        let mut file = std::fs::File::create(&pending)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&pending, &path)?;
        std::fs::File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(pending);
    }
    result
}

#[cfg(test)]
pub(super) fn load(root: Option<&Path>, host: &str) -> crate::Result<Saved> {
    let workspace = load_workspace(root, host, "default")?;
    Ok(workspace
        .tabs
        .into_iter()
        .nth(workspace.active)
        .map_or_else(Saved::default, |tab| tab.layout))
}

#[cfg(test)]
pub(super) fn save(root: Option<&Path>, host: &str, saved: &Saved) -> crate::Result<()> {
    save_value(
        root,
        host,
        "default",
        serde_json::json!({
            "version": 1, "host": host, "session": "default",
            "ids": saved.ids.iter().map(TerminalId::as_str).collect::<Vec<_>>(),
            "active": saved.active, "vertical": saved.vertical
        }),
    )
}
