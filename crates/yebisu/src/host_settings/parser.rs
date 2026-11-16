//! Parse only allowlisted fields; never retain private input or parser diagnostics.
use super::{Error, FileState, MAX_BYTES};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KnownSettings {
    /// None means absent in this file, not an inferred daemon default.
    pub pane_history: Option<bool>,
}

/// Transport must enforce the same byte limit before accumulating a response.
pub(crate) fn parse(bytes: Option<&[u8]>) -> FileState {
    match bytes {
        None => FileState::Missing,
        Some(bytes) => match known(bytes) {
            Ok(value) => FileState::Parsed(value),
            Err(error) => FileState::Failed(error),
        },
    }
}
fn known(bytes: &[u8]) -> Result<KnownSettings, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
    let document: toml::Table = toml::from_str(text).map_err(|_| Error::InvalidToml)?;
    let experimental = document
        .get("experimental")
        .map(|value| value.as_table().ok_or(Error::InvalidField))
        .transpose()?;
    let pane_history = experimental
        .and_then(|table| table.get("pane_history"))
        .map(|value| value.as_bool().ok_or(Error::InvalidField))
        .transpose()?;
    Ok(KnownSettings { pane_history })
}
