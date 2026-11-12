//! Local-only theme selection. No host input, Follow mode, filesystem or UI writer.
//! The catalog deliberately contains only palettes implemented by the consumer.
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Palette {
    #[default]
    OwnerDefault,
}

pub(crate) const SUPPORTED: &[Palette] = &[Palette::OwnerDefault];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    UnsupportedPalette,
    InvalidSettings,
}
impl Palette {
    pub fn key(self) -> &'static str {
        match self {
            Self::OwnerDefault => "owner-default",
        }
    }
    pub fn from_key(key: &str) -> Result<Self, Error> {
        match key {
            "owner-default" => Ok(Self::OwnerDefault),
            _ => Err(Error::UnsupportedPalette),
        }
    }
}

/// Absence preserves Theme::default. Unsupported persisted data is not reset.
pub(crate) fn read_local(document: &serde_json::Value) -> Result<Palette, Error> {
    let object = document.as_object().ok_or(Error::InvalidSettings)?;
    match object.get("theme") {
        None => Ok(Palette::default()),
        Some(value) => Palette::from_key(value.as_str().ok_or(Error::InvalidSettings)?),
    }
}

/// Pure patch for the future SettingsHost-owned serialization path. Never writes IO.
/// A failed patch leaves the input unchanged; unrelated and unknown keys survive.
pub(crate) fn patch_local(document: &mut serde_json::Value, palette: Palette) -> Result<(), Error> {
    let object = document.as_object_mut().ok_or(Error::InvalidSettings)?;
    object.insert(
        "theme".into(),
        serde_json::Value::String(palette.key().into()),
    );
    Ok(())
}
