//! Whether a Herdr 2 terminal runs something Close Space would interrupt
//! (#231, owner decision 2026-11-26): a program or an agent asks first; a
//! shell alone closes at once.
//!
//! A terminal runs something when it reported a status of its own (an
//! agent's OSC 7501) or when its foreground process group's leader (else
//! the program it was started with) is not a shell. The shells are the
//! official app's (`herdr-app/defaults/programs.json` `shells`), and a name
//! is read as its `base` does (`herdr-app/src/programs.rs`): the last path
//! part, without a login shell's leading `-` or a Windows `.exe`.
use sheprd_core_client::wire::{Terminal, TerminalState};

// keep-sorted start
const SHELLS: [&str; 15] = [
    "bash",
    "cmd",
    "csh",
    "dash",
    "elvish",
    "fish",
    "ion",
    "ksh",
    "nu",
    "powershell",
    "pwsh",
    "sh",
    "tcsh",
    "xonsh",
    "zsh",
];
// keep-sorted end

/// A program name as the official app compares it.
fn base(name: &str) -> &str {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let name = name.trim_start_matches('-');
    match name.len().checked_sub(4) {
        Some(i) if name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".exe") => &name[..i],
        _ => name,
    }
}

/// Whether `name` is one of the shells.
pub(crate) fn is_shell(name: &str) -> bool {
    SHELLS.contains(&base(name))
}

/// Whether the live `terminal` runs a program or an agent. An ended one
/// runs nothing; one that names no program runs its default shell.
pub(crate) fn runs_program(terminal: &Terminal) -> bool {
    if terminal.state != TerminalState::Live {
        return false;
    }
    if terminal.status.is_some() {
        return true;
    }
    let program = terminal
        .foreground
        .as_ref()
        .and_then(|fg| fg.argv.first().cloned().or_else(|| Some(fg.name.clone())))
        .filter(|name| !name.is_empty())
        .or_else(|| terminal.argv.first().cloned());
    program.is_some_and(|name| !is_shell(&name))
}

#[cfg(test)]
#[path = "running_tests.rs"]
mod tests;
