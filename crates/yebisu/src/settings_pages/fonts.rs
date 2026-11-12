//! Terminal font density backed by the persisted `compact` setting.

use crate::settings::Values;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Density {
    Regular,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TerminalFont {
    pub density: Density,
    pub size_px: f32,
    pub line_height_px: f32,
}

impl Density {
    pub(crate) const ALL: [Self; 2] = [Self::Regular, Self::Compact];

    pub(crate) const fn from_values(values: &Values) -> Self {
        if values.compact {
            Self::Compact
        } else {
            Self::Regular
        }
    }

    pub(crate) const fn metrics(self) -> TerminalFont {
        match self {
            Self::Regular => TerminalFont {
                density: self,
                size_px: 14.0,
                line_height_px: 20.0,
            },
            Self::Compact => TerminalFont {
                density: self,
                size_px: 12.0,
                line_height_px: 17.0,
            },
        }
    }

    /// Edits the real SettingsHost draft field; the host still owns saving.
    pub(crate) fn apply(self, values: &mut Values) {
        values.compact = matches!(self, Self::Compact);
    }
}

/// The only bundled face currently consumed by the terminal painter.
pub(crate) const TERMINAL_FAMILY: &str = "SauceCodePro Nerd Font Mono";
