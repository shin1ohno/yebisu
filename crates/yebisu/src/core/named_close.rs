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
//!
//! A `space-<ms>` space this connection does not serve closes too (#249 B):
//! the discovery poll's snapshot (`discover.py`, with each terminal's
//! foreground) gives the question and the live terminals seen, and the stop
//! (`stop.py`), in its one checked connection, ends those and refuses if a
//! live terminal it was not told of is there. A server the poll has seen for
//! less than `vacate::GRACE` is not closed: another client may have just
//! created it.
use super::{running, vacate};
use sheprd_core_client::{
    CoreBackend,
    wire::{Terminal, TerminalState},
};
use std::{collections::BTreeMap, time::Instant};

/// Why a close is refused: something started running since the question.
pub(crate) const STARTED_SINCE: &str = "A program started in this space; choose Close Space again";
/// Why `default` is refused: its server is never stopped (`vacate.rs`).
pub(crate) const PROTECTED: &str = "The default space can't be closed";

/// What the fresh list says before anything ends: the live terminals to
/// end, or why the close is refused.
pub(super) fn plan(
    name: &str,
    asked: &[String],
    terminals: &[Terminal],
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

/// A `space-<ms>` space this connection does not serve, as the discovery
/// poll last saw it (#249 B): its server, its live terminals (which a
/// Close ends, `vacate::Expect::end`), the busy ones among them (which the
/// question names), and since when the poll has seen this same server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) expect: vacate::Expect,
    pub(crate) busy: Vec<String>,
    pub(crate) since: Instant,
}

/// Why a listed space cannot close: the poll has no answer from its server.
pub(crate) const NOT_ANSWERING: &str = "This space isn't answering; try again";
/// Why a listed space cannot close yet: its server was first seen less than
/// `vacate::GRACE` ago, so another client may have just created it.
pub(crate) const JUST_STARTED: &str = "This space just started; try again in a moment";

/// Records one poll's answering spaces at `now`. A space keeps the time its
/// server was first seen while the same server (core id and pid) answers;
/// another server starts again, and a space that did not answer is dropped.
pub(super) fn observe_listed(
    listed: &mut BTreeMap<String, Listed>,
    now: Instant,
    seen: Vec<(String, String, Option<u64>, Vec<Terminal>)>,
) {
    let mut next = BTreeMap::new();
    for (name, core_id, pid, terminals) in seen {
        if vacate::protected(&name) {
            continue;
        }
        let since = listed
            .get(&name)
            .filter(|old| old.expect.core_id == core_id && old.expect.pid == pid)
            .map_or(now, |old| old.since);
        let mut expect = vacate::Expect::seen(core_id, pid);
        expect.end = terminals
            .iter()
            .filter(|terminal| terminal.state == TerminalState::Live)
            .map(|terminal| (terminal.address.clone(), judged_pid(terminal)))
            .collect();
        next.insert(
            name,
            Listed {
                expect,
                busy: running::busy(&terminals),
                since,
            },
        );
    }
    *listed = next;
}

/// The pid of what a question judged a terminal by (`running::runs_program`):
/// its foreground program, else the program it was started with, whose pid
/// is the terminal's own. A foreground reported only later (herdr-core fills
/// it in after a terminal starts) is then the same program, not a change.
pub(super) fn judged_pid(terminal: &Terminal) -> Option<i32> {
    terminal
        .foreground
        .as_ref()
        .map(|fg| fg.pid)
        .or(terminal.pid)
}

/// Whether the listed space `listed` may close at `now`: its server
/// answered, and the poll has seen this same server for `vacate::GRACE`
/// (the rule that keeps another client's just-created space, `vacate.rs`).
pub(crate) fn listed_closable(listed: Option<&Listed>, now: Instant) -> Result<(), &'static str> {
    let listed = listed.ok_or(NOT_ANSWERING)?;
    if now.saturating_duration_since(listed.since) < vacate::GRACE {
        return Err(JUST_STARTED);
    }
    Ok(())
}

/// Closes the listed space `name` through the checked stop, which ends the
/// terminals in `expect.end` and refuses if another live terminal is there,
/// or a seen one's foreground program changed since the question
/// (`Busy`): the poll's `retire_vacant`, which forgets the space once its
/// server stopped. A refusal or a failure is the screen's notice. Blocking:
/// the worker runs it on a thread of its own.
#[cfg(any(target_os = "ios", test))]
pub(super) fn close_listed(
    lists: &super::SessionList,
    root: Option<&std::path::Path>,
    host: &str,
    space: &(String, vacate::Expect),
    stop: &vacate::Stopper,
) -> crate::Result<vacate::Stopped> {
    let stopped = super::poll::retire_vacant(lists, root, host, space, stop)?;
    let mut lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match stopped {
        vacate::Stopped::Busy => {
            lists.notice = Some(format!(
                "Could not remove Herdr 2 space {}: {STARTED_SINCE}",
                space.0
            ));
        }
        vacate::Stopped::Done => {
            lists.discovery.listed.remove(&space.0);
        }
    }
    Ok(stopped)
}

/// Runs `close_listed` on a thread of its own, so the worker loop never
/// waits for SSH; its outcome reaches the screen through `inbox`.
pub(super) fn spawn_close_listed(
    (lists, inbox): (
        super::SessionList,
        std::sync::Arc<std::sync::Mutex<crate::live::Live>>,
    ),
    (root, host): (Option<std::path::PathBuf>, String),
    space: (String, vacate::Expect),
    stop: vacate::Stopper,
) -> crate::Result<()> {
    #[cfg(any(target_os = "ios", test))]
    {
        std::thread::Builder::new()
            .name("yebisu-core-close".into())
            .spawn(move || {
                let result = close_listed(&lists, root.as_deref(), &host, &space, &stop);
                if let Err(error) = &result {
                    tracing::warn!(%error, space = %space.0, "Herdr 2 Close Space");
                }
                let notice = lists
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .notice
                    .clone();
                if let Ok(mut live) = inbox.lock() {
                    if let Some(notice) = notice {
                        live.status = notice;
                    }
                    live.dirty = true;
                }
            })
            .map(drop)
            .map_err(|error| crate::Error::Ssh(format!("Close Space: {error}")))
    }
    #[cfg(not(any(target_os = "ios", test)))]
    {
        let _ = (lists, inbox, root, host, space, stop);
        Err(crate::Error::Ssh(
            "SSH unavailable on this host: cannot close a Herdr 2 space".into(),
        ))
    }
}

#[cfg(test)]
#[path = "named_close_tests.rs"]
mod tests;
