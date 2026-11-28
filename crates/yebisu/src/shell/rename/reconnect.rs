//! The rename field across a reconnect (#270, #143 D13).
//!
//! Going to another app and back pauses the connection and dials a new one
//! (a new `Link::generation`). The field stays open with its text while the
//! connection is away; once the new connection's snapshot arrives, the
//! target is checked against it: the same boot with the object still in it
//! moves the target to the new generation, so a commit is sent there; another
//! boot (Herdr restarted) or a missing object closes the field. The boot is
//! never mixed: a target keeps the boot it was opened on, and is moved only
//! to a connection whose snapshot is of that boot.
use super::{RenameTarget, herdr2::DocNames};
use crate::{app::Smoke, board::snapshot, live::Live, shell::HerdrKind};

/// What the connection says about the open field's target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    /// Connected on the target's generation, with a snapshot: the frame
    /// tick checks the boot and the object as before.
    Current,
    /// Paused, reconnecting, or no snapshot yet: the field stays as it is.
    Waiting,
    /// The new connection's snapshot is of the same boot and holds the
    /// object: the target moved to this generation.
    Rebound,
    /// The new connection's snapshot is of another boot or lacks the
    /// object: the field closes with this text.
    Gone(&'static str),
}

/// A Herdr 1 target against a connection's state: `None` while there is
/// nothing to check it against yet.
fn check(live: &Live, target: &RenameTarget) -> Option<Result<(), &'static str>> {
    if !live.connected || live.version_mismatch.get().is_some() {
        return None;
    }
    let shown = snapshot(live)?;
    if shown.boot_id != target.boot || !target.object.exists_in(shown) {
        return Some(Err(target.object.closed_text()));
    }
    Some(Ok(()))
}

/// A Herdr 2 target: the synthesized snapshot's boot, and the document the
/// drawn `default` session read (`herdr2.rs`).
fn check_doc(
    live: &Live,
    doc: Option<&DocNames>,
    target: &RenameTarget,
) -> Option<Result<(), &'static str>> {
    if !live.connected {
        return None;
    }
    let shown = snapshot(live)?;
    if shown.boot_id != target.boot {
        return Some(Err(target.object.closed_text()));
    }
    Some(if doc?.holds(&target.object) {
        Ok(())
    } else {
        Err(target.object.closed_text())
    })
}

impl Smoke {
    /// Checks the open field's target against the connection now, and moves
    /// it to a new connection of the same boot. The host is checked first by
    /// the caller: another selected host closes the field.
    pub(crate) fn rebind_rename(&mut self) -> Bound {
        let Some(target) = self.shell.rename.session.as_ref().map(|s| s.target.clone()) else {
            return Bound::Waiting;
        };
        if self.terminal.reconnect.suspended {
            return Bound::Waiting;
        }
        let generation = self.terminal.link.generation;
        if target.generation == generation {
            let live = &self.terminal.live;
            return if live.connected && snapshot(live).is_some() {
                Bound::Current
            } else {
                Bound::Waiting
            };
        }
        let verdict = match target.kind {
            // The inbox, not the UI's copy: it is replaced with the
            // generation (`Link::pause`), so its snapshot is the new
            // connection's, as `send_rename` reads it.
            HerdrKind::Herdr1 => match self.terminal.link.inbox.try_lock() {
                Ok(live) => check(&live, &target),
                Err(_) => None,
            },
            HerdrKind::Herdr2 => check_doc(&self.terminal.live, self.doc_names().as_ref(), &target),
        };
        match verdict {
            None => Bound::Waiting,
            Some(Err(reason)) => Bound::Gone(reason),
            Some(Ok(())) => {
                if let Some(session) = self.shell.rename.session.as_mut() {
                    session.target.generation = generation;
                    session.missed = 0;
                }
                self.observation.record(serde_json::json!({
                    "kind": "rename-rebound",
                    "generation": generation,
                }));
                Bound::Rebound
            }
        }
    }
}
