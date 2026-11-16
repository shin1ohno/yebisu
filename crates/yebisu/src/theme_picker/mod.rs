//! Local-only theme selection. No host input, Follow mode, filesystem or UI writer.
//! The catalog deliberately contains only palettes implemented by the consumer.
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Palette {
    #[default]
    OwnerDefault,
    HighContrast,
}

pub(crate) const SUPPORTED: &[Palette] = &[Palette::OwnerDefault, Palette::HighContrast];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    UnsupportedPalette,
    InvalidSettings,
}
impl Palette {
    pub fn key(self) -> &'static str {
        match self {
            Self::OwnerDefault => "owner-default",
            Self::HighContrast => "high-contrast",
        }
    }
    pub fn from_key(key: &str) -> Result<Self, Error> {
        match key {
            "owner-default" => Ok(Self::OwnerDefault),
            "high-contrast" => Ok(Self::HighContrast),
            _ => Err(Error::UnsupportedPalette),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OwnerDefault => "Owner default",
            Self::HighContrast => "High contrast",
        }
    }

    /// Optional role override; ANSI palette and owner default are unchanged.
    pub fn colors(self) -> Option<Colors> {
        match self {
            Self::OwnerDefault => None,
            Self::HighContrast => Some(Colors {
                background: 0x000000,
                foreground: 0xffffff,
                muted: 0xa0a0a0,
                primary: 0x00ffff,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Colors {
    pub background: u32,
    pub foreground: u32,
    pub muted: u32,
    pub primary: u32,
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
