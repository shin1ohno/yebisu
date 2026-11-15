//! A fixed reviewed reader, never a general command API. No dispatch capability.
use super::{ConfigPath, Error};
use std::fmt::Write as _;

pub(super) const READER: &str = include_str!("reader.py");
pub(super) const MAX_COMMAND_BYTES: usize = 16384;

pub(super) fn command(path: &ConfigPath) -> Result<String, Error> {
    // Only the compiled-in literal is shell escaped. The data alphabet is hex.
    let literal = READER.replace('\'', "'\\''");
    let mut command = format!("exec /usr/bin/python3 -I -S -B -c '{literal}' '");
    for byte in path.as_str().bytes() {
        write!(command, "{byte:02x}").map_err(|_| Error::CommandTooLarge)?;
    }
    command.push('\'');
    if command.len() > MAX_COMMAND_BYTES {
        return Err(Error::CommandTooLarge);
    }
    Ok(command)
}
