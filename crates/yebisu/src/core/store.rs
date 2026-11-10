//! One host/session record per file; no remote setting or browser state is written.
pub(super) static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use sheprd_backend::TerminalId;
use std::{
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct Saved {
    pub ids: Vec<TerminalId>,
    pub active: usize,
    pub vertical: bool,
}
fn path(root: &Path, host: &str) -> PathBuf {
    // Hex is injective, unlike sanitizing punctuation in SSH target names.
    let name: String = host.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    root.join(format!("core-{name}-default.json"))
}
pub(super) fn load(root: Option<&Path>, host: &str) -> crate::Result<Saved> {
    let Some(root) = root else {
        return Ok(Saved::default());
    };
    let file = match std::fs::File::open(path(root, host)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Saved::default()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(crate::Error::CoreState);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| crate::Error::CoreState)?;
    if value["version"] != 1 || value["host"] != host || value["session"] != "default" {
        return Err(crate::Error::CoreState);
    }
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
    let active = value["active"].as_u64().ok_or(crate::Error::CoreState)? as usize;
    if !ids.is_empty() && active >= ids.len() {
        return Err(crate::Error::CoreState);
    }
    Ok(Saved {
        ids,
        active,
        vertical: value["vertical"].as_bool().ok_or(crate::Error::CoreState)?,
    })
}
pub(super) fn save(root: Option<&Path>, host: &str, state: &Saved) -> crate::Result<()> {
    let Some(root) = root else {
        return Ok(());
    };
    std::fs::create_dir_all(root)?;
    let path = path(root, host);
    let pending = path.with_extension("pending");
    let result = (|| {
        let mut file = std::fs::File::create(&pending)?;
        let value = serde_json::json!({"version":1,"host":host,"session":"default","ids":state.ids.iter().map(TerminalId::as_str).collect::<Vec<_>>(),"active":state.active,"vertical":state.vertical});
        file.write_all(value.to_string().as_bytes())?;
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
