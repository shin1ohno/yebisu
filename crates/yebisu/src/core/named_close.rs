//! Close Space on a `space-<ms>` session, the one this connection serves
//! (#249, owner decision 2026-11-27). Such a space is its own herdr-core
//! server, so closing it ends every live terminal of that server; the
//! worker then finds the space vacant (its last tab gone, nothing live) and
//! stops the server through `vacate.rs`'s checked stop, as when its last tab
//! closes. No other stop path is added here.
//!
//! The view asks first from the status poll's busy terminals, which may be a
//! second old (`status.rs`). The worker takes a fresh list before it ends
//! anything: a terminal that runs a program or an agent and was not in the
//! question refuses the close, so nothing the user was not asked about ends.
//! `default` is never closed this way.
use super::{running, vacate};
use sheprd_core_client::{CoreBackend, wire::TerminalState};

/// Why a close is refused: something started running since the question.
pub(crate) const STARTED_SINCE: &str = "A program started in this space; choose Close Space again";
/// Why `default` is refused: its server is never stopped (`vacate.rs`).
pub(crate) const PROTECTED: &str = "The default space can't be closed";

/// What the fresh list says before anything ends: the live terminals to
/// end, or why the close is refused.
pub(super) fn plan(
    name: &str,
    asked: &[String],
    terminals: &[sheprd_core_client::wire::Terminal],
) -> crate::Result<Vec<String>> {
    if vacate::protected(name) {
        return Err(crate::Error::Ssh(PROTECTED.into()));
    }
    if running::busy(terminals)
        .iter()
        .any(|address| !asked.contains(address))
    {
        return Err(crate::Error::Ssh(STARTED_SINCE.into()));
    }
    Ok(terminals
        .iter()
        .filter(|terminal| terminal.state == TerminalState::Live)
        .map(|terminal| terminal.address.clone())
        .collect())
}

/// Ends the live terminals of the session `name` that `backend` serves.
/// Blocking: the worker thread only.
pub(super) fn end_terminals(
    backend: &CoreBackend,
    name: &str,
    asked: &[String],
) -> crate::Result<()> {
    let list = backend.core().list(false)?;
    let ending = plan(name, asked, &list.terminals)?;
    let mut failed = 0;
    for address in &ending {
        if let Err(error) = backend.core().kill(address) {
            tracing::warn!(%error, %name, "Herdr 2 Close Space kill");
            failed += 1;
        }
    }
    if failed > 0 {
        return Err(crate::Error::Ssh(format!(
            "Could not end {failed} of the space's terminals"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "named_close_tests.rs"]
mod tests;
