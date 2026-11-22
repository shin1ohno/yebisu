//! Herdr [keys] bindings resolved to the existing iPad action catalog.
mod host;
use gpui::{Keystroke, Modifiers};
use sheprd_commands::Command;
use std::sync::LazyLock;

pub(crate) use host::HostKeys;

const DEFAULT_PREFIX: &str = "ctrl+b";
const MAX_BYTES: usize = 64 * 1024;
const MAX_BINDINGS_PER_ACTION: usize = 16;

// Herdr 1 config/model.rs and Sheprd's keymap/daemon.rs. A split_vertical
// divides the panes with a vertical border (the new pane is on the right).
const BINDINGS: &[(&str, Command, &str)] = &[
    ("new_tab", Command::Tab, "prefix+c"),
    ("split_vertical", Command::SplitRight, "prefix+v"),
    ("split_horizontal", Command::SplitDown, "prefix+minus"),
    ("next_tab", Command::NextTab, "prefix+n"),
    ("previous_tab", Command::PreviousTab, "prefix+p"),
    ("close_tab", Command::CloseTab, "prefix+shift+x"),
    ("help", Command::Keybinds, "prefix+?"),
    ("focus_pane_left", Command::FocusLeft, "prefix+h"),
    ("focus_pane_right", Command::FocusRight, "prefix+l"),
    ("focus_pane_up", Command::FocusUp, "prefix+k"),
    ("focus_pane_down", Command::FocusDown, "prefix+j"),
    ("cycle_pane_next", Command::NextPane, "prefix+tab"),
    (
        "cycle_pane_previous",
        Command::PreviousPane,
        "prefix+shift+tab",
    ),
    ("close_pane", Command::ClosePane, "prefix+x"),
    ("zoom", Command::Zoom, "prefix+z"),
    ("toggle_sidebar", Command::ToggleSidebar, "prefix+b"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fallback {
    Disconnected,
    Loading,
    #[cfg(any(not(target_os = "ios"), target_abi = "sim"))]
    Unavailable,
    MissingFile,
    TooLarge,
    InvalidUtf8,
    InvalidToml,
    MissingPrefix,
    InvalidPrefix,
    Timeout,
    NotRegular,
    ReadFailed,
    PermissionDenied,
}

impl Fallback {
    fn reason(self) -> &'static str {
        match self {
            Self::Disconnected => "No connected host.",
            Self::Loading => "Reading the connected host's Herdr config.",
            #[cfg(any(not(target_os = "ios"), target_abi = "sim"))]
            Self::Unavailable => "The host config is unavailable in this session.",
            Self::MissingFile => "The host's Herdr config file is missing.",
            Self::TooLarge => "The host's Herdr config exceeds 64 KiB.",
            Self::InvalidUtf8 => "The host's Herdr config is not UTF-8.",
            Self::InvalidToml => "The host's Herdr config is not valid TOML.",
            Self::MissingPrefix => "[keys].prefix is missing.",
            Self::InvalidPrefix => "[keys].prefix has no usable key.",
            Self::Timeout => "Reading the host's Herdr config timed out.",
            Self::NotRegular => "The host’s Herdr config is not a regular file.",
            Self::PermissionDenied => "Permission denied reading the host’s Herdr config.",
            Self::ReadFailed => "The host's Herdr config could not be read.",
        }
    }
}

#[derive(Debug)]
struct Binding {
    key: Keystroke,
    command: Command,
    prefixed: bool,
    label: String,
}

#[derive(Debug)]
struct Profile {
    prefixes: Vec<(Keystroke, String)>,
    bindings: Vec<Binding>,
    fallback: Option<Fallback>,
}

static DEFAULTS: LazyLock<Profile> = LazyLock::new(|| Profile::from_keys(None));

impl Profile {
    fn from_file(file: Result<&[u8], Fallback>) -> Result<Self, Fallback> {
        let bytes = file?;
        if bytes.len() > MAX_BYTES {
            return Err(Fallback::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Fallback::InvalidUtf8)?;
        let config = text
            .parse::<toml::Table>()
            .map_err(|_| Fallback::InvalidToml)?;
        Ok(Self::from_keys(
            config.get("keys").and_then(toml::Value::as_table),
        ))
    }

    fn from_keys(keys: Option<&toml::Table>) -> Self {
        let value = keys.and_then(|table| table.get("prefix"));
        let mut prefixes: Vec<_> = entries(value)
            .into_iter()
            .flatten()
            .take(MAX_BINDINGS_PER_ACTION)
            .filter_map(|label| {
                let key = parse_key(label)?;
                // Terminal input cannot replay the platform modifier as a key.
                (!key.modifiers.platform).then(|| (key, label.trim().to_owned()))
            })
            .collect();
        let fallback = if prefixes.is_empty() {
            prefixes.extend(parse_key(DEFAULT_PREFIX).map(|key| (key, DEFAULT_PREFIX.to_owned())));
            Some(if value.is_none() {
                Fallback::MissingPrefix
            } else {
                Fallback::InvalidPrefix
            })
        } else {
            None
        };
        let mut configured = Vec::new();
        let mut defaults = Vec::new();
        for &(name, command, default) in BINDINGS {
            let value = keys.and_then(|table| table.get(name));
            if let Some(chords) = entries(value) {
                configured.extend(
                    chords
                        .into_iter()
                        .take(MAX_BINDINGS_PER_ACTION)
                        .filter_map(|chord| binding(command, chord)),
                );
            } else {
                defaults.extend(binding(command, default));
            }
        }
        let tabs = keys.and_then(|table| table.get("switch_tab"));
        let (tab_chords, bindings) = match entries(tabs) {
            Some(chords) => (chords, &mut configured),
            None => (vec!["prefix+1..9"], &mut defaults),
        };
        for chord in tab_chords.into_iter().take(MAX_BINDINGS_PER_ACTION) {
            if chord.split('+').any(|part| part.trim() == "1..9") {
                for digit in 1..=9 {
                    let expanded = chord.replace("1..9", &digit.to_string());
                    bindings.extend(binding(Command::TabNumber(digit), &expanded));
                }
            } else if let Some(mut binding) = binding(Command::TabNumber(1), chord)
                && let Ok(digit @ 1..=9) = binding.key.key.parse::<u8>()
            {
                binding.command = Command::TabNumber(digit);
                bindings.push(binding);
            }
        }
        configured.extend(defaults);
        Self {
            prefixes,
            bindings: configured,
            fallback,
        }
    }

    fn prefix(&self, key: &Keystroke) -> bool {
        self.prefixes.iter().any(|(prefix, _)| matches(prefix, key))
    }

    fn command(&self, key: &Keystroke, prefixed: bool) -> Option<Command> {
        self.bindings
            .iter()
            .find(|binding| binding.prefixed == prefixed && matches(&binding.key, key))
            .map(|binding| binding.command)
    }
}

fn entries(value: Option<&toml::Value>) -> Option<Vec<&str>> {
    match value? {
        toml::Value::String(value) => Some(vec![value]),
        toml::Value::Array(values) => values.iter().map(toml::Value::as_str).collect(),
        _ => None,
    }
}

fn binding(command: Command, text: &str) -> Option<Binding> {
    let text = text.trim();
    let (prefixed, label) = text
        .strip_prefix("prefix+")
        .map_or((false, text), |key| (true, key));
    let key = parse_key(label)?;
    // Herdr rejects bare printable direct bindings so ordinary typing stays in the pane.
    if !prefixed
        && key.key.chars().count() == 1
        && !key.modifiers.control
        && !key.modifiers.alt
        && !key.modifiers.platform
    {
        return None;
    }
    Some(Binding {
        key,
        command,
        prefixed,
        label: label.to_owned(),
    })
}

fn parse_key(text: &str) -> Option<Keystroke> {
    let text = text.trim();
    let (modifiers, name) = if text == "+" {
        ("", "+")
    } else if let Some(modifiers) = text.strip_suffix("++") {
        (modifiers, "+")
    } else {
        text.rsplit_once('+').unwrap_or(("", text))
    };
    let mut key = Keystroke {
        key: name.trim().to_ascii_lowercase(),
        ..Default::default()
    };
    for modifier in modifiers.split('+').filter(|_| !modifiers.is_empty()) {
        match modifier.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => key.modifiers.control = true,
            "shift" => key.modifiers.shift = true,
            "alt" | "option" | "meta" => key.modifiers.alt = true,
            "cmd" | "command" | "super" => key.modifiers.platform = true,
            _ => return None,
        }
    }
    let alias = match key.key.as_str() {
        "esc" => "escape",
        "return" => "enter",
        "bs" => "backspace",
        "backtab" => {
            key.modifiers.shift = true;
            "tab"
        }
        "minus" => "-",
        "comma" => ",",
        "period" => ".",
        "slash" => "/",
        "backslash" => "\\",
        "quote" => "'",
        "double_quote" | "double-quote" => "\"",
        "semicolon" => ";",
        "colon" => ":",
        "percent" => "%",
        "ampersand" => "&",
        "backtick" => "`",
        "plus" => "+",
        "space" | "tab" | "enter" | "escape" | "backspace" | "delete" | "insert" | "home"
        | "end" | "pageup" | "pagedown" | "left" | "right" | "up" | "down" => "",
        _ if key.key.chars().count() == 1 && !key.key.starts_with(char::is_control) => {
            key.modifiers.shift |= name.trim().starts_with(|c: char| c.is_ascii_uppercase());
            ""
        }
        _ if key
            .key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n)) =>
        {
            ""
        }
        _ => return None,
    };
    if !alias.is_empty() {
        key.key = alias.to_owned();
    }
    Some(key)
}

fn matches(binding: &Keystroke, key: &Keystroke) -> bool {
    if binding.key == key.key && binding.modifiers == key.modifiers {
        return true;
    }
    if !binding.modifiers.shift
        || key.modifiers
            != (Modifiers {
                shift: false,
                ..binding.modifiers
            })
    {
        return false;
    }
    // UIKit reports shifted punctuation as the resulting character with Shift cleared.
    const BASE: &str = "`1234567890-=[]\\;',./";
    const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
    binding.key.len() == 1
        && key.key.len() == 1
        && BASE.bytes().zip(SHIFTED.bytes()).any(|(base, shifted)| {
            binding.key.as_bytes() == [base] && key.key.as_bytes() == [shifted]
        })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Native,
    Terminal,
    Replay,
    Consume(Option<Command>),
}

#[derive(Default)]
pub(crate) struct Prefix {
    armed: Option<Keystroke>,
}

impl Prefix {
    pub(crate) fn clear(&mut self) {
        self.armed = None;
    }

    fn route(
        &mut self,
        profile: &Profile,
        prefixes: &Profile,
        key: &Keystroke,
        held: bool,
    ) -> Route {
        if held {
            return Route::Terminal; // Earlier app-owned presses are fenced by the key handler.
        }
        if let Some(armed) = self.armed.take() {
            return if matches(&armed, key) {
                Route::Replay
            } else {
                Route::Consume(profile.command(key, true))
            };
        }
        if prefixes.prefix(key) {
            self.armed = Some(key.clone());
            return Route::Consume(None);
        }
        match profile.command(key, false) {
            Some(command) => Route::Consume(Some(command)),
            None => Route::Terminal,
        }
    }
}

#[cfg(test)]
mod tests;
