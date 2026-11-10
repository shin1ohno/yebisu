// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/terminal_painter/glyphs.rs at 398c132a06723a0cc1494842a34666d173ec0200
//! Shaped cell symbols, reused across frames.
//!
//! Only the font, its size and bold/italic change a symbol's glyphs; color is
//! applied when painting. Keying by color would shape one symbol once per
//! color, so truecolor output would exhaust the bounded cache and reshape
//! every frame. ASCII, most of any grid, is found by index without hashing.

use crate::terminal::{BOLD, ITALIC};
use gpui::{Font, FontStyle, ShapedLine};
use std::collections::HashMap;

pub(super) const CACHE_LIMIT: usize = 4096;
// A cell holds one grapheme, not a line. 128 UTF-8 bytes allows 32
// maximum-width scalars (ZWJ emoji, variation selectors and combining marks).
// Longer pathological graphemes are deliberately refused before UI shaping.
pub(crate) const SYMBOL_BYTES: usize = 128;
pub(super) const CACHE_BYTES: usize = 16 * 1024 * 1024;
const STYLES: usize = 4;

pub(crate) fn valid_symbol(symbol: &str) -> bool {
    symbol.len() <= SYMBOL_BYTES && !symbol.chars().any(char::is_control)
}

// Charge both text copies, shaped allocations and conservative entry overhead.
// Fixed ASCII slots/table headers are bounded separately by CACHE_LIMIT.
fn entry_bytes(symbol: &str, line: &ShapedLine) -> usize {
    size_of::<ShapedLine>()
        + size_of::<gpui::LineLayout>()
        + 2 * symbol.len()
        + 64
        + line.runs.capacity() * size_of::<gpui::ShapedRun>()
        + line
            .runs
            .iter()
            .map(|run| run.glyphs.capacity() * size_of::<gpui::ShapedGlyph>())
            .sum::<usize>()
}

pub(super) struct GlyphCache {
    /// A shaped line is kilobytes, so the tables below hold indexes into this.
    lines: Vec<ShapedLine>,
    pub(super) bytes: usize,
    /// `ascii[style * 128 + byte]`.
    ascii: Vec<Option<u32>>,
    other: [HashMap<String, u32>; STYLES],
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            bytes: 0,
            ascii: vec![None; STYLES * 128],
            other: Default::default(),
        }
    }
}

/// The modifier bits that change shaping, as an index.
pub(super) fn style(modifier: u16) -> usize {
    usize::from(modifier & BOLD != 0) | usize::from(modifier & ITALIC != 0) << 1
}

pub(super) fn style_modifier(style: usize) -> u16 {
    (if style & 1 != 0 { BOLD } else { 0 }) | if style & 2 != 0 { ITALIC } else { 0 }
}

/// The face a cached glyph `style` shapes in: bold cells draw a step heavier
/// than the configured weight rather than at a fixed bold.
pub(super) fn styled_font(font: &Font, style: usize) -> Font {
    let mut font = font.clone();
    let modifier = style_modifier(style);
    if modifier & BOLD != 0 {
        font.weight = crate::theme::bold_font_weight(font.weight);
    }
    if modifier & ITALIC != 0 {
        font.style = FontStyle::Italic;
    }
    font
}

fn ascii_index(style: usize, symbol: &str) -> Option<usize> {
    match symbol.as_bytes() {
        [byte] if byte.is_ascii() => Some(style * 128 + usize::from(*byte)),
        _ => None,
    }
}

impl GlyphCache {
    pub(super) fn get(&self, style: usize, symbol: &str) -> Option<&ShapedLine> {
        let id = match ascii_index(style, symbol) {
            Some(index) => self.ascii[index]?,
            None => *self.other[style].get(symbol)?,
        };
        self.lines.get(usize::try_from(id).ok()?)
    }

    /// Past either budget, paint the shaped line once without retaining it.
    pub(super) fn insert(&mut self, style: usize, symbol: &str, line: ShapedLine) -> bool {
        if !valid_symbol(symbol) || self.get(style, symbol).is_some() {
            return false;
        }
        let bytes = entry_bytes(symbol, &line);
        if self.lines.len() >= CACHE_LIMIT || bytes > CACHE_BYTES.saturating_sub(self.bytes) {
            return false;
        }
        self.bytes += bytes;
        // CACHE_LIMIT fits in u32; `has_room` keeps the count below it.
        let id = u32::try_from(self.lines.len()).unwrap_or(u32::MAX);
        match ascii_index(style, symbol) {
            Some(index) => self.ascii[index] = Some(id),
            None => {
                self.other[style].insert(symbol.to_owned(), id);
            }
        }
        self.lines.push(line);
        true
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.lines.len()
    }

    /// Every cached symbol with its style index.
    #[cfg(test)]
    pub(super) fn iter(&self) -> impl Iterator<Item = (usize, String, &ShapedLine)> {
        let ascii = self.ascii.iter().enumerate().filter_map(|(index, id)| {
            let byte = u8::try_from(index % 128).ok()?;
            let symbol = char::from(byte).to_string();
            Some((
                index / 128,
                symbol,
                self.lines.get(usize::try_from((*id)?).ok()?)?,
            ))
        });
        let other = self.other.iter().enumerate().flat_map(move |(style, ids)| {
            ids.iter().filter_map(move |(symbol, id)| {
                Some((
                    style,
                    symbol.clone(),
                    self.lines.get(usize::try_from(*id).ok()?)?,
                ))
            })
        });
        ascii.chain(other)
    }
}
