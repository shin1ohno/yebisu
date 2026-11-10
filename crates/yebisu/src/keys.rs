//! One display/copy boundary for every installation key, including future replacement.
// Modified from shin1ohno/herdr-gpui crates/sheprd-client/src/ssh/ipad.rs at 398c132a06723a0cc1494842a34666d173ec0200
use crate::{Error, Result};

const NAME_LIMIT: usize = 48;

pub(crate) fn authorized_line(line: &str) -> Result<String> {
    if line.len() > 4096 || line.chars().any(char::is_control) {
        return Err(Error::InvalidKeyLine);
    }
    let mut fields = line.splitn(4, ' ');
    let restriction = fields.next().ok_or(Error::InvalidKeyLine)?;
    let kind = fields.next().ok_or(Error::InvalidKeyLine)?;
    let blob = fields.next().ok_or(Error::InvalidKeyLine)?;
    let comment = fields.next().ok_or(Error::InvalidKeyLine)?;
    if restriction != "restrict"
        || !matches!(kind, "ecdsa-sha2-nistp256" | "ssh-ed25519" | "ssh-rsa")
        || blob.is_empty()
        || !blob
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
    {
        return Err(Error::InvalidKeyLine);
    }
    let device = if comment == "herdr-ipad" {
        ""
    } else {
        comment
            .strip_prefix("herdr-ipad-")
            .ok_or(Error::InvalidKeyLine)?
    };
    let name: String = device
        .trim()
        .trim_end_matches(".local")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') {
                c
            } else {
                '-'
            }
        })
        .take(NAME_LIMIT)
        .collect();
    let name = name.trim_matches('-');
    let comment = if name.is_empty() {
        "yebisu".to_owned()
    } else {
        format!("yebisu-{name}")
    };
    Ok(format!("restrict {kind} {blob} {comment}"))
}

#[cfg(test)]
mod tests;
