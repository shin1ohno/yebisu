// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/osc52.rs at 398c132a06723a0cc1494842a34666d173ec0200
//! OSC 52 clipboard writes the daemon forwards to this client.
//!
//! A program inside a pane can ask to set the system clipboard with an OSC 52
//! escape. Herdr intercepts that escape in the pane's PTY and, since this
//! client owns no terminal to write it to, sends the bytes to the foreground
//! client as the base64 `ServerMessage::Clipboard`. Only text is written here:
//! the payload is bounded, must decode as base64, and must be valid UTF-8.
//! Anything else is dropped rather than guessed at, so a malformed or hostile
//! escape cannot replace the clipboard with bytes this client never agreed to
//! write.

/// The largest decoded write this client will put on the pasteboard, matching
/// the bound applied to clipboard *reads*.
pub(crate) const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// How many decoded writes may wait for the UI thread. Every write replaces
/// the whole clipboard, so only the tail can still matter; the bound keeps a
/// spamming pane from growing the mailbox.
pub(crate) const MAX_PENDING: usize = 8;

/// Decodes one `ServerMessage::Clipboard` payload.
///
/// The daemon encodes the raw bytes with standard base64. Padding is optional
/// and either alphabet is accepted, but any other character, data after
/// padding, a non-canonical tail, invalid UTF-8, or a decoded payload past
/// [`MAX_TEXT_BYTES`] is rejected.
pub(crate) fn decode(data: &str) -> Option<String> {
    let bytes = decode_base64(data)?;
    if bytes.len() > MAX_TEXT_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn decode_base64(data: &str) -> Option<Vec<u8>> {
    // Reject an oversized payload before allocating for it. Four encoded
    // symbols carry at most three bytes.
    if data.len() > MAX_TEXT_BYTES.div_ceil(3) * 4 + 4 {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    let mut padding = 0usize;
    for &byte in data.as_bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' if padding < 2 => {
                padding += 1;
                continue;
            }
            _ => return None,
        };
        if padding != 0 {
            return None;
        }
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    // The final symbol may pad its quantum with zero bits, never data bits.
    if buffer != 0 {
        return None;
    }
    // Padding, when present, must account for exactly the symbols the last
    // quantum is short by; a lone leftover symbol is never valid.
    let symbols = data.len() - padding;
    if symbols % 4 == 1 || (padding != 0 && symbols % 4 != 4 - padding) {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests;
