//! The terminal keys over the software keyboard (#229): Blink's shortcuts
//! bar with alt added, and a … menu for the rest. One list serves the
//! keyboard's own bar (`bar`) and the in-app band drawn where the keyboard
//! has no bar (a floating keyboard).

/// What a key does. Control, Alt and Command latch for the next key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Key(&'static str),
    Text(&'static str),
    Control,
    Alt,
    Command,
    Paste,
    /// The host's Herdr prefix (its first `[keys].prefix`).
    Prefix,
    Hide,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Key {
    /// The stable name: the QA identifier is `key-band-<name>` and the bar
    /// item's id is the key's index in `ALL`.
    pub(crate) name: &'static str,
    /// The in-app band's label.
    pub(crate) label: &'static str,
    /// The keyboard bar's SF Symbol; `None` shows `label`.
    pub(crate) symbol: Option<&'static str>,
    pub(crate) accessibility_label: &'static str,
    pub(crate) action: Action,
}

const fn key(
    name: &'static str,
    label: &'static str,
    symbol: Option<&'static str>,
    accessibility_label: &'static str,
    action: Action,
) -> Key {
    Key {
        name,
        label,
        symbol,
        accessibility_label,
        action,
    }
}

/// Every key once. Groups below index into this list.
pub(crate) const ALL: [Key; 23] = [
    key("ctrl", "ctrl", Some("control"), "Control", Action::Control),
    key("alt", "alt", Some("option"), "Alt", Action::Alt),
    key(
        "esc",
        "esc",
        Some("escape"),
        "Escape",
        Action::Key("escape"),
    ),
    key(
        "tab",
        "tab",
        Some("arrow.right.to.line"),
        "Tab",
        Action::Key("tab"),
    ),
    key("backtick", "`", None, "Backtick", Action::Text("`")),
    key("caret", "^", None, "Caret", Action::Text("^")),
    key("backslash", "\\", None, "Backslash", Action::Text("\\")),
    key("bracket-left", "[", None, "Left bracket", Action::Text("[")),
    key(
        "bracket-right",
        "]",
        None,
        "Right bracket",
        Action::Text("]"),
    ),
    key("less", "<", None, "Less than", Action::Text("<")),
    key("greater", ">", None, "Greater than", Action::Text(">")),
    key(
        "left",
        "←",
        Some("arrowtriangle.left"),
        "Left arrow",
        Action::Key("left"),
    ),
    key(
        "up",
        "↑",
        Some("arrowtriangle.up"),
        "Up arrow",
        Action::Key("up"),
    ),
    key(
        "down",
        "↓",
        Some("arrowtriangle.down"),
        "Down arrow",
        Action::Key("down"),
    ),
    key(
        "right",
        "→",
        Some("arrowtriangle.right"),
        "Right arrow",
        Action::Key("right"),
    ),
    key("cmd", "⌘", Some("command"), "Command", Action::Command),
    key(
        "paste",
        "Paste",
        Some("doc.on.clipboard"),
        "Paste",
        Action::Paste,
    ),
    key(
        "prefix",
        "prefix",
        Some("command.square"),
        "Herdr prefix",
        Action::Prefix,
    ),
    key("pipe", "|", None, "Vertical bar", Action::Text("|")),
    key("tilde", "~", None, "Tilde", Action::Text("~")),
    key("slash", "/", None, "Slash", Action::Text("/")),
    key("minus", "-", None, "Hyphen", Action::Text("-")),
    key(
        "hide",
        "",
        Some("keyboard.chevron.compact.down"),
        "Hide keyboard",
        Action::Hide,
    ),
];

/// The bar's leading groups (Blink: the modifiers, then the symbols).
pub(crate) const LEADING: [&[usize]; 2] = [&[0, 1], &[2, 3, 4, 5, 6, 7, 8, 9, 10]];
/// The bar's trailing groups: the arrows, then ⌘.
pub(crate) const TRAILING: [&[usize]; 2] = [&[11, 12, 13, 14], &[15]];
/// The … menu at the bar's trailing end.
pub(crate) const MENU: [usize; 7] = [16, 17, 18, 19, 20, 21, 22];

/// The … button's QA identifier and label.
pub(crate) const MORE_NAME: &str = "more";
pub(crate) const MORE_SYMBOL: &str = "ellipsis.circle";
pub(crate) const MORE_LABEL: &str = "More keys";

/// The key with bar item `id`, if any.
pub(crate) fn by_id(id: u32) -> Option<&'static Key> {
    ALL.get(usize::try_from(id).ok()?)
}

/// The bar item id of `ALL[index]`.
pub(crate) fn id(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

/// The in-app band's keys in order: the bar's, then the menu's inline (the
/// band has room to scroll, and no menu of its own).
pub(crate) fn band_order() -> impl Iterator<Item = &'static Key> {
    LEADING
        .iter()
        .chain(TRAILING.iter())
        .flat_map(|group| group.iter())
        .chain(MENU.iter())
        .filter_map(|&index| ALL.get(index))
}
