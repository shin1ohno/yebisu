//! Ended terminals out of the `herdr-app` document: their panes, then the
//! tabs and the spaces that leaves empty.
//!
//! The official app keeps its model true to herdr-core on every listing: a
//! pane whose terminal finished as `exited` or `killed`, or that herdr-core
//! no longer knows (`not_found`), is removed, a tab left without panes goes,
//! and a space left without tabs goes, and the document is written again
//! (`herdr-app/src/window.rs` `reconcile`, `checked_closed`; `model.rs`
//! `remove_pane`, `remove_tab`, `remove_space`). yebisu draws only the tabs
//! with a live terminal, so a space whose terminals all ended was drawn with
//! no tab at all, yet the sidebar still listed it with its tab count; and a
//! space with one live terminal still counted its ended tabs. yebisu now
//! removes them the same way (`herdr_doc_prune`, owner answer 2026-11-26).
//!
//! A live terminal's pane is never removed. A wrong removal wipes the
//! owner's layout, so a pane goes only when:
//!
//! - the document and the terminal list both came from one read, on one
//!   connection (a connection is one herdr-core boot: a core that restarts
//!   ends it), the document before the list, so every terminal it names was
//!   made before the list was taken;
//! - that read is complete: a readable document and a list that answered
//!   (a failed read forgets every candidate);
//! - its terminal finished as `exited` or `killed`, or as `server_stop`
//!   while not kept, or is absent from the full list and belongs to this
//!   machine (an address of another machine, `box/t7`, is never judged
//!   absent here);
//! - its tab holds nothing but terminal blocks in rows and columns (a tab
//!   with a stack, a floating block, a mod or an agent stays whole);
//! - a later read of the same boot, at least one poll period after the
//!   first, finds the same space byte for byte with the same ended
//!   terminals. The first read of a boot never removes.
//!
//! A server stop ends only a terminal that is not kept: herdr-core 0.1.13
//! lists it `finished`/`server_stop` after the next start, and its program
//! never comes back, while a kept one is `live` again with a shell
//! (`stopped_by_restart`), as the real-core test `ended_live_tests` reads.
//! The official app keeps the first pane too; yebisu removes it, so what an
//! `hc server stop` ended goes as well. `holder_lost` and any other reason
//! are not ends.
use super::{
    herdr_doc::{Intent, View},
    herdr_doc_prune::{plain, prune_space},
};
use serde_json::Value;
use sheprd_core_client::wire::{Terminal, TerminalState};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    time::{Duration, Instant},
};

/// The reasons the official app takes as "closed" (`checked_closed`).
const CLOSED: [&str; 2] = ["exited", "killed"];
/// The server stopped under it (`hc server stop`, a restart).
const STOPPED: &str = "server_stop";

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
                && match terminal.reason.as_deref() {
                    Some(STOPPED) => !terminal.keep,
                    Some(reason) => CLOSED.contains(&reason),
                    None => false,
                };
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

/// A space of the document with something to remove.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Candidate {
    pub(crate) id: String,
    /// The space as read.
    pub(crate) space: Value,
    /// Its ended terminals in plain tabs, sorted.
    pub(crate) ended: Vec<String>,
}

/// The spaces of `value` that pruning their ended terminals changes. `view`
/// is `value` parsed.
pub(crate) fn ended(value: &Value, view: &View, listing: &Listing) -> Vec<Candidate> {
    let spaces = value.get("spaces").and_then(Value::as_array);
    view.spaces
        .iter()
        .enumerate()
        .filter(|(_, space)| !space.id.is_empty())
        .filter_map(|(i, space)| {
            let raw = spaces?.get(i)?;
            let tabs = raw.get("tabs").and_then(Value::as_array);
            let ended = tabs
                .into_iter()
                .flatten()
                .zip(&space.tabs)
                .filter(|(tab, _)| tab.get("root").is_none_or(plain))
                .flat_map(|(_, tab)| &tab.terminals)
                .filter(|address| listing.ended(address))
                .cloned()
                .collect::<BTreeSet<_>>();
            // A space without tabs is left over: the official app removes
            // a space with its last tab.
            let empty = tabs.is_none_or(Vec::is_empty);
            (empty || prune_space(raw, &ended) != *raw).then(|| Candidate {
                id: space.id.clone(),
                space: raw.clone(),
                ended: ended.into_iter().collect(),
            })
        })
        .collect()
}

/// The candidates of one connection, kept from read to read.
#[derive(Debug)]
pub(crate) struct Tracker {
    /// How long a candidate stays before its prune is due: the document's
    /// poll period (`DOC_POLL`), so two reads see it.
    grace: Duration,
    /// The boot the candidates were seen on.
    boot: Option<String>,
    /// Each candidate as first seen, and when.
    seen: BTreeMap<String, (Candidate, Instant)>,
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

    /// Takes one complete read of `boot` at `now` and returns the prunes
    /// that are due: candidates seen on this boot for the grace or more,
    /// unchanged since.
    pub(crate) fn observe(
        &mut self,
        boot: &str,
        now: Instant,
        found: Vec<Candidate>,
    ) -> Vec<Intent> {
        if self.boot.as_deref() != Some(boot) {
            self.boot = Some(boot.to_owned());
            self.seen.clear();
        }
        let ids = found
            .iter()
            .map(|candidate| candidate.id.clone())
            .collect::<HashSet<_>>();
        self.seen.retain(|id, _| ids.contains(id));
        let mut due = Vec::new();
        for candidate in found {
            match self.seen.get(&candidate.id) {
                Some((first, at)) if *first == candidate => {
                    if now.saturating_duration_since(*at) >= self.grace {
                        due.push(Intent::PruneSpace {
                            id: candidate.id,
                            space: candidate.space,
                            ended: candidate.ended,
                        });
                    }
                }
                _ => {
                    self.seen.insert(candidate.id.clone(), (candidate, now));
                }
            }
        }
        due
    }
}
