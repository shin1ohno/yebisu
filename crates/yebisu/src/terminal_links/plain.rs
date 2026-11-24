//! Addresses and paths read from a pane's text. Web addresses are found as
//! the fork finds them (`plain_url`: read wherever `http://` or `https://`
//! stands, trimmed of the prose punctuation around them); the other words a
//! press may mean to open are named only so the press can say why they do
//! not open.
use super::{Refusal, Target};
use std::ops::Range;

/// The longest address accepted, the bound the fork's `WebUrl` keeps.
const MAX_URL_BYTES: usize = 8192;

/// The schemes read from plain text as web addresses.
const WEB_SCHEMES: [&str; 2] = ["http://", "https://"];

/// Schemes read from plain text without `//`, to refuse them by name.
const BARE_SCHEMES: [&str; 8] = [
    "data",
    "file",
    "javascript",
    "mailto",
    "sms",
    "tel",
    "vbscript",
    "vscode",
];

/// Characters that never belong to a host: the URL standard's forbidden host
/// code points that can appear in printed text.
const FORBIDDEN_HOST: [char; 9] = ['<', '>', '"', '\\', '^', '`', '{', '|', '}'];

/// `value` itself when it is a bounded `http` or `https` address with a
/// host, and nothing in it a browser would read differently from how the
/// pane shows it: no blank and no control character.
pub(super) fn web(value: &str) -> Option<String> {
    if value.len() > MAX_URL_BYTES || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    host(value).map(|_| value.to_owned())
}

/// The host of an `http` or `https` address, as in `example.com`: what a
/// status line names, since the path and query may hold a secret.
pub(crate) fn host(value: &str) -> Option<&str> {
    let (scheme, rest) = value.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.strip_prefix('[') {
        // An IPv6 literal, with an optional port after the bracket.
        Some(literal) => literal.split_once(']')?.0,
        None => host.split(':').next().unwrap_or_default(),
    };
    (!host.is_empty() && !host.contains(FORBIDDEN_HOST)).then_some(host)
}

/// The scheme `value` starts with, as the URL standard reads one: a letter,
/// then letters, digits, `+`, `-` or `.`, then `:`.
pub(super) fn scheme(value: &str) -> Option<&str> {
    let (scheme, _) = value.split_once(':')?;
    let mut chars = scheme.chars();
    (chars.next()?.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
    .then_some(scheme)
}

/// Where a web address starts in `text`, in order.
fn address_starts(text: &str) -> Vec<(usize, &'static str)> {
    let mut starts: Vec<_> = WEB_SCHEMES
        .into_iter()
        .flat_map(|scheme| {
            text.match_indices(scheme)
                .map(move |(start, _)| (start, scheme))
        })
        .collect();
    starts.sort_unstable();
    starts
}

/// Whether `c` ends a word of printed text.
fn separates(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\'' | '`')
}

/// `candidate` without the prose punctuation after it, and without closing
/// brackets it does not open, as in `(see https://a.test/x).`.
fn trimmed(candidate: &str) -> &str {
    let mut candidate = candidate.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
        let excess = candidate
            .matches(close)
            .count()
            .saturating_sub(candidate.matches(open).count());
        for _ in 0..excess {
            let Some(shorter) = candidate.strip_suffix(close) else {
                break;
            };
            candidate = shorter;
        }
    }
    candidate
}

/// The byte range of the web address in `text` that covers the byte at
/// `hit`, trimmed of the prose punctuation around it, and whether the
/// address runs to the end of the text, where it may go on.
pub(super) fn plain_url(text: &str, hit: usize) -> Option<(Range<usize>, bool)> {
    for (start, scheme) in address_starts(text) {
        if start > hit {
            break;
        }
        let tail = &text[start..];
        let end = tail.find(separates).unwrap_or(tail.len());
        // The original token decides: punctuation at the edge may be part of
        // a destination that goes on past the text read.
        let open = end == tail.len();
        let candidate = trimmed(&tail[..end]);
        // A scheme with nothing after it names nothing.
        if candidate.len() <= scheme.len() {
            continue;
        }
        if hit < start + candidate.len() {
            return Some((start..start + candidate.len(), open));
        }
        if open {
            return None;
        }
    }
    None
}

/// A link read from text: what it points at, the bytes it covers, and
/// whether it runs to the end of the text.
pub(super) struct Found {
    pub(super) target: Target,
    pub(super) range: Range<usize>,
    pub(super) open: bool,
}

/// The word of `text` around the byte at `hit`, trimmed like an address.
fn word(text: &str, hit: usize) -> Option<(Range<usize>, bool)> {
    if !text.is_char_boundary(hit) {
        return None;
    }
    let start = text[..hit].rfind(separates).map_or(0, |at| {
        at + text[at..].chars().next().map_or(1, char::len_utf8)
    });
    let end = text[hit..]
        .find(separates)
        .map_or(text.len(), |at| hit + at);
    let candidate = trimmed(text.get(start..end)?);
    (!candidate.is_empty() && hit < start + candidate.len())
        .then(|| (start..start + candidate.len(), end == text.len()))
}

/// Whether a printed word names a file: it starts as a path does, or names a
/// folder.
fn path_like(word: &str) -> bool {
    word.starts_with('/')
        || word.starts_with("~/")
        || word.starts_with("./")
        || word.starts_with("../")
        || word.contains('/')
}

/// What `text` may link to at the byte at `hit`. A web address there is
/// read first, as in the fork; otherwise a word with another scheme, or a
/// path, is named so the press can refuse it.
pub(super) fn read(text: &str, hit: usize) -> Option<Found> {
    if let Some((range, open)) = plain_url(text, hit) {
        let target = Target::Web(web(text.get(range.clone())?)?);
        return Some(Found {
            target,
            range,
            open,
        });
    }
    let (range, open) = word(text, hit)?;
    let found = text.get(range.clone())?;
    let refusal = match scheme(found) {
        // A web scheme in capitals, which the fork leaves as text, or a web
        // address without a host.
        Some(name) if name.eq_ignore_ascii_case("http") || name.eq_ignore_ascii_case("https") => {
            return Some(Found {
                target: web(found).map_or(Target::Refused(Refusal::Malformed), Target::Web),
                range,
                open,
            });
        }
        Some(name) if name.eq_ignore_ascii_case("file") => Refusal::Path,
        Some(name)
            if found[name.len()..].starts_with("://")
                || BARE_SCHEMES.contains(&name.to_ascii_lowercase().as_str()) =>
        {
            Refusal::Scheme(name.to_ascii_lowercase())
        }
        _ if path_like(found) => Refusal::Path,
        _ => return None,
    };
    Some(Found {
        target: Target::Refused(refusal),
        range,
        open,
    })
}
