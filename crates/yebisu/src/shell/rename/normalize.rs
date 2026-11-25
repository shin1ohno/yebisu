//! The name a rename sends: one line, trimmed, at most 160 characters.

use crate::shell::sidebar::model::CAPTION_LIMIT;

/// The most characters (Rust `char`s) a name keeps: what the sidebar shows.
pub(crate) const MAX_CHARS: usize = CAPTION_LIMIT;

/// Control characters and line breaks become spaces, the ends are trimmed,
/// and the name is cut to `MAX_CHARS` characters.
pub(crate) fn normalize_name(text: &str) -> String {
    let line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    line.trim().chars().take(MAX_CHARS).collect()
}
