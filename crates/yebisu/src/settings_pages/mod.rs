//! Source-adapted page models for the production SettingsHost sheet.
//!
//! This module deliberately contains no sheet owner, persistence worker, or
//! platform credential operation. The SettingsHost integrator owns those.

pub(crate) mod device_key;
pub(crate) mod fonts;
pub(crate) mod general;
pub(crate) mod keyboard;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Fonts,
    Keyboard,
    DeviceKey,
    General,
}

impl Page {
    pub(crate) const ALL: [Self; 4] = [Self::Fonts, Self::Keyboard, Self::DeviceKey, Self::General];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Fonts => "fonts",
            Self::Keyboard => "keyboard",
            Self::DeviceKey => "ipad-key",
            Self::General => "general",
        }
    }
}
