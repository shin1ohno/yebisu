//! Herdr 2 spaces of the `herdr-app` document whose every terminal ended.
//!
//! The official app keeps its model true to herdr-core on every listing: a
//! pane whose terminal finished as `exited` or `killed`, or that herdr-core
//! no longer knows (`not_found`), is removed, a tab left without panes goes,
//! and a space left without tabs goes, and the document is written again
//! (`herdr-app/src/window.rs` `reconcile`, `checked_closed`; `model.rs`
//! `remove_pane`, `remove_tab`, `remove_space`). yebisu draws only the tabs
//! with a live terminal, so a space whose terminals all ended was drawn with
//! no tab at all, yet the sidebar still listed it with its tab count.
//!
//! yebisu removes such a space whole, and nothing smaller: a space with one
//! live terminal keeps every tab. A wrong removal wipes the owner's layout,
//! so a space goes only when:
//!
//! - the document and the terminal list both came from one read, on one
//!   connection (a connection is one herdr-core boot: a core that restarts
//!   ends it), the document after the list's terminals were made;
//! - that read is complete: a readable document and a list that answered
//!   (a failed read forgets every candidate);
//! - each terminal the space names finished as `exited` or `killed`, or is
//!   absent from the full list and belongs to this machine (an address of
//!   another machine, `box/t7`, is never judged absent here);
//! - no tab holds anything but terminal blocks (a mod or an agent block has
//!   no terminal that could end);
//! - a later read of the same boot, at least one poll period after the
//!   first, finds the same space byte for byte. The first read of a boot
//!   never removes.
//!
//! `server_stop` and `holder_lost` are not ends: herdr-core gives a kept
//! terminal a shell at its next start, so the official app keeps its pane.
use super::herdr_doc::{Intent, View};
use serde_json::Value;
use sheprd_core_client::wire::{Terminal, TerminalState};
use std::{
    collections::{BTreeMap, HashSet},
    time::{Duration, Instant},
};

/// The reasons the official app takes as "closed" (`checked_closed`).
const CLOSED: [&str; 2] = ["exited", "killed"];

/// One full `terminal.list` (`all: true`): every terminal herdr-core knows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Listing {
    closed: HashSet<String>,
    known: HashSet<String>,
}

impl Listing {
    pub(crate) fn new(terminals: &[Terminal]) -> Self {
        let mut listing = Self::default();
        for terminal in terminals {
            let closed = terminal.state == TerminalState::Finished
                && terminal
                    .reason
                    .as_deref()
                    .is_some_and(|reason| CLOSED.contains(&reason));
            if closed {
                listing.closed.insert(terminal.address.clone());
            }
            listing.known.insert(terminal.address.clone());
        }
        listing
    }

    fn ended(&self, address: &str) -> bool {
        self.closed.contains(address) || (!self.known.contains(address) && !address.contains('/'))
    }
}

/// The spaces of `value` whose every terminal ended in `listing`, each with
/// its id and its value as read. `view` is `value` parsed.
pub(crate) fn ended(value: &Value, view: &View, listing: &Listing) -> Vec<(String, Value)> {
    let spaces = value.get("spaces").and_then(Value::as_array);
    view.spaces
        .iter()
        .enumerate()
        .filter(|(_, space)| !space.id.is_empty())
        .filter(|(_, space)| {
            space
                .tabs
                .iter()
                .all(|tab| !tab.other && tab.terminals.iter().all(|address| listing.ended(address)))
        })
        .filter_map(|(i, space)| {
            let raw = spaces?.get(i)?.clone();
            Some((space.id.clone(), raw))
        })
        .collect()
}

/// The candidates of one connection, kept from read to read.
#[derive(Debug)]
pub(crate) struct Tracker {
    /// How long a space stays ended before it is removed: the document's
    /// poll period (`DOC_POLL`), so two reads see it.
    grace: Duration,
    /// The boot the candidates were seen on.
    boot: Option<String>,
    /// Each candidate as first seen, and when.
    seen: BTreeMap<String, (Value, Instant)>,
}

impl Tracker {
    pub(crate) fn new(grace: Duration) -> Self {
        Self {
            grace,
            boot: None,
            seen: BTreeMap::new(),
        }
    }

    /// A failed or partial read: nothing it did not see may be removed.
    pub(crate) fn forget(&mut self) {
        self.seen.clear();
    }

    /// Takes one complete read of `boot` at `now` and returns the removals
    /// that are due: candidates seen ended on this boot for the grace or more,
    /// unchanged since.
    pub(crate) fn observe(
        &mut self,
        boot: &str,
        now: Instant,
        found: Vec<(String, Value)>,
    ) -> Vec<Intent> {
        if self.boot.as_deref() != Some(boot) {
            self.boot = Some(boot.to_owned());
            self.seen.clear();
        }
        let ids = found
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        self.seen.retain(|id, _| ids.contains(id));
        let mut due = Vec::new();
        for (id, space) in found {
            match self.seen.get(&id) {
                Some((first, at)) if *first == space => {
                    if now.saturating_duration_since(*at) >= self.grace {
                        due.push(Intent::RemoveSpace {
                            id: id.clone(),
                            space,
                        });
                    }
                }
                _ => {
                    self.seen.insert(id, (space, now));
                }
            }
        }
        due
    }
}
