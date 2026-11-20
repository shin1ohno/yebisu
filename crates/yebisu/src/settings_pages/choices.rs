//! L1: the radio rows of the Input page (six, Q7) and the Connection page
//! (Return, q7). Each row is a two-way choice over one `Values` field; the
//! labels are the canvas text (`Settings.dc.html`).
use crate::settings::Values;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    OptionKey,
    CommandKey,
    ShortcutPriority,
    JapaneseCandidates,
    StartSelection,
    Copy,
    Return,
}

/// The Input page's rows, in canvas order.
pub(crate) const INPUT: [Choice; 6] = [
    Choice::OptionKey,
    Choice::CommandKey,
    Choice::ShortcutPriority,
    Choice::JapaneseCandidates,
    Choice::StartSelection,
    Choice::Copy,
];

pub(crate) const HARDWARE_KEYBOARD: &[Choice] = INPUT.split_at(4).0;
pub(crate) const SELECTION: &[Choice] = INPUT.split_at(4).1;

/// The Connection page's rows.
pub(crate) const CONNECTION: [Choice; 1] = [Choice::Return];

impl Choice {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::OptionKey => "option-key",
            Self::CommandKey => "command-key",
            Self::ShortcutPriority => "shortcut-priority",
            Self::JapaneseCandidates => "japanese-candidates",
            Self::StartSelection => "start-selection",
            Self::Copy => "copy",
            Self::Return => "return",
        }
    }

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::OptionKey => "Option key",
            Self::CommandKey => "Command key",
            Self::ShortcutPriority => "Shortcut priority",
            Self::JapaneseCandidates => "Japanese candidates",
            Self::StartSelection => "Start a selection",
            Self::Copy => "Copy",
            Self::Return => "Return",
        }
    }

    pub(crate) const fn detail(self) -> &'static str {
        match self {
            Self::OptionKey => "Native types characters; Alt sends Alt to the terminal.",
            Self::CommandKey => "Local keeps ⌘ for iPad shortcuts; Ctrl sends ⌘ chords as Control.",
            Self::ShortcutPriority => {
                "With Command as Ctrl, choose whether ⌘ chords run app actions first."
            }
            Self::JapaneseCandidates => "Where the conversion window opens.",
            Self::StartSelection => "Default: drag on the terminal, as in other iPad apps.",
            Self::Copy => "Default: copy with the edit bar or ⌘C.",
            Self::Return => "When the app returns, reconnect automatically or wait for a tap.",
        }
    }

    /// The labels for `false` and `true`.
    pub(crate) const fn labels(self) -> [&'static str; 2] {
        match self {
            Self::OptionKey => ["Native", "Alt"],
            Self::CommandKey => ["Local", "Ctrl"],
            Self::ShortcutPriority => ["App", "Terminal"],
            Self::JapaneseCandidates => ["At the clause", "At the caret"],
            // `drag_select` is true for Drag, so Drag is the `true` label.
            Self::StartSelection => ["Double tap", "Drag"],
            Self::Copy => ["Manual", "On selection"],
            Self::Return => ["Tap", "Automatic"],
        }
    }

    pub(crate) const fn options(self) -> [(bool, &'static str); 2] {
        let [off, on] = self.labels();
        match self {
            Self::StartSelection => [(true, on), (false, off)],
            _ => [(false, off), (true, on)],
        }
    }

    pub(crate) const fn get(self, values: &Values) -> bool {
        match self {
            Self::OptionKey => values.option_alt,
            Self::CommandKey => values.command_control,
            Self::ShortcutPriority => super::keyboard::terminal_priority(values),
            Self::JapaneseCandidates => values.caret_anchor,
            Self::StartSelection => values.drag_select,
            Self::Copy => values.copy_on_select,
            Self::Return => values.reconnect_automatic,
        }
    }

    pub(crate) fn set(self, values: &mut Values, value: bool) {
        match self {
            Self::OptionKey => values.option_alt = value,
            Self::CommandKey => values.command_control = value,
            Self::ShortcutPriority => {
                super::keyboard::set_terminal_priority(values, value);
            }
            Self::JapaneseCandidates => values.caret_anchor = value,
            Self::StartSelection => values.drag_select = value,
            Self::Copy => values.copy_on_select = value,
            Self::Return => values.reconnect_automatic = value,
        }
    }
}
