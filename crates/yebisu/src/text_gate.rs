//! Why `accepts_text_input` answers as it does, as one enum value. GPUI asks
//! every frame and herdr-uikit makes the text input view first responder
//! only while the answer is yes, so a lasting no explains typing that goes
//! nowhere. Each change of answer goes to the diagnostics log as a static
//! code (`diagnostics::record`); no key, text or host is recorded.
use std::cell::Cell;

/// The inputs of the decision, sampled by `accepts_text_input`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Facts {
    pub(crate) settings_modal: bool,
    /// A terminal is open and no sheet covers it (`terminal_owns_text_input`).
    pub(crate) terminal_owns: bool,
    /// The find bar is open or its handoff is paused.
    pub(crate) find: bool,
    /// The active focus handle (the terminal's, or the sheet's) is focused.
    pub(crate) focused: bool,
    /// The terminal has a destination for input (`TerminalInput::destination`).
    pub(crate) destination: bool,
    pub(crate) navigating: bool,
    pub(crate) palette: bool,
    pub(crate) shortcuts: bool,
    /// A sheet's input is free: smoke mode, or setup is not busy.
    pub(crate) sheet_ready: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    Settings,
    Find,
    Focus,
    Destination,
    Navigating,
    Palette,
    Shortcuts,
    /// A sheet owns text input and is not focused or busy.
    Sheet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    Accepts,
    Off(Reason),
}

impl Gate {
    /// The same order `accepts_text_input` has always checked; the first
    /// failing condition is the reason.
    pub(crate) fn decide(facts: Facts) -> Self {
        let off = if facts.settings_modal {
            Some(Reason::Settings)
        } else if facts.terminal_owns {
            if facts.find {
                Some(Reason::Find)
            } else if !facts.focused {
                Some(Reason::Focus)
            } else if !facts.destination {
                Some(Reason::Destination)
            } else if facts.navigating {
                Some(Reason::Navigating)
            } else if facts.palette {
                Some(Reason::Palette)
            } else if facts.shortcuts {
                Some(Reason::Shortcuts)
            } else {
                None
            }
        } else if !facts.focused || !facts.sheet_ready {
            Some(Reason::Sheet)
        } else {
            None
        };
        off.map_or(Self::Accepts, Self::Off)
    }

    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Accepts => "text_on",
            Self::Off(Reason::Settings) => "text_off_settings",
            Self::Off(Reason::Find) => "text_off_find",
            Self::Off(Reason::Focus) => "text_off_focus",
            Self::Off(Reason::Destination) => "text_off_destination",
            Self::Off(Reason::Navigating) => "text_off_navigating",
            Self::Off(Reason::Palette) => "text_off_palette",
            Self::Off(Reason::Shortcuts) => "text_off_shortcuts",
            Self::Off(Reason::Sheet) => "text_off_sheet",
        }
    }
}

/// Every code `Gate::code` returns, for the diagnostics allowlist.
pub(crate) const CODES: &[&str] = &[
    "text_on",
    "text_off_settings",
    "text_off_find",
    "text_off_focus",
    "text_off_destination",
    "text_off_navigating",
    "text_off_palette",
    "text_off_shortcuts",
    "text_off_sheet",
];

/// The last answer recorded, so a code is written only when it changes.
#[derive(Default)]
pub(crate) struct Recorder(Cell<Option<Gate>>);

impl Recorder {
    /// The code to record for `gate`, if it differs from the last one.
    pub(crate) fn changed(&self, gate: Gate) -> Option<&'static str> {
        (self.0.replace(Some(gate)) != Some(gate)).then(|| gate.code())
    }

    pub(crate) fn note(&self, gate: Gate) {
        if let Some(code) = self.changed(gate) {
            crate::diagnostics::record(code);
        }
    }
}

#[cfg(test)]
#[path = "text_gate_tests.rs"]
mod tests;
