//! Source-adapted page models for the production SettingsHost sheet.
//!
//! This module deliberately contains no sheet owner, persistence worker, or
//! platform credential operation. The SettingsHost integrator owns those.

pub(crate) mod choices;
pub(crate) mod device_key;
pub(crate) mod fonts;
pub(crate) mod general;
pub(crate) mod keyboard;
// SH1-157 L1 (docs/layout-contracts.md, plan v2 7.4).
pub(crate) mod sections;
pub(crate) mod sheet_layout;
pub(crate) mod toolbar_sync;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Fonts,
    Appearance,
    Keyboard,
    DeviceKey,
    General,
    Notifications,
    // SH1-157 L1: the pages of the Input and Connection sections (q7).
    Input,
    Connection,
}

impl Page {
    // The sheet walks sections::GROUPS since L1; only tests list every page.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 8] = [
        Self::Fonts,
        Self::Appearance,
        Self::Keyboard,
        Self::DeviceKey,
        Self::General,
        Self::Notifications,
        Self::Input,
        Self::Connection,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Fonts => "fonts",
            Self::Appearance => "appearance",
            Self::Keyboard => "keyboard",
            Self::DeviceKey => "ipad-key",
            Self::General => "general",
            Self::Notifications => "notifications",
            Self::Input => "input",
            Self::Connection => "connection",
        }
    }
}
