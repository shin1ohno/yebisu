use super::Values;
use crate::{Error, Result};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::Path,
};

pub(super) fn load(root: &Path) -> Result<Values> {
    let file = match fs::File::open(root.join("yebisu-settings.json")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Values::default()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(Error::Settings("file exceeds 4096 bytes".into()));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| Error::Settings(e.to_string()))?;
    if value["version"] != 1 || value["language"] != "en-US" {
        return Err(Error::Settings("unsupported schema or language".into()));
    }
    let boolean = |key| {
        value[key]
            .as_bool()
            .ok_or_else(|| Error::Settings(format!("invalid {key}")))
    };
    let optional_boolean = |key, default| match value.get(key) {
        None => Ok(default),
        Some(v) => v
            .as_bool()
            .ok_or_else(|| Error::Settings(format!("invalid {key}"))),
    };
    Ok(Values {
        font: super::font::TerminalFont::from_json(&value)?,
        compact: boolean("compact")?,
        terminal_priority: boolean("terminal_priority")?,
        board_hidden: boolean("board_hidden")?,
        drag_select: optional_boolean("drag_select", true)?,
        copy_on_select: optional_boolean("copy_on_select", false)?,
    })
}

pub(super) fn save(root: &Path, values: &Values) -> Result<()> {
    values.font.validate()?;
    fs::create_dir_all(root)?;
    let pending = root.join("yebisu-settings.pending");
    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&pending)?;
        file.write_all(
            serde_json::json!({"version":1,"language":"en-US",
            "terminal_font_size":values.font.size,"terminal_font_family":values.font.family,
            "compact":values.compact,"terminal_priority":values.terminal_priority,
            "board_hidden":values.board_hidden,
            "drag_select":values.drag_select,"copy_on_select":values.copy_on_select})
            .to_string()
            .as_bytes(),
        )?;
        file.sync_all()?;
        fs::rename(&pending, root.join("yebisu-settings.json"))?;
        fs::File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
}
