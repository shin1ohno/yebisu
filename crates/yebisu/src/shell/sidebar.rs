//! L3: hosts > spaces > AGENTS. Owner: L3.
//!
//! This file holds the contract items (docs/layout-contracts.md section 5)
//! and the receive-order memory. `model` builds host, space and agent rows
//! from daemon snapshots; `target` resolves notification targets by
//! identity; `view` draws the rows and routes taps.
use super::{HerdrKind, sidebar_status::SidebarStatus};
use std::{cell::RefCell, cmp::Ordering, collections::BTreeMap};

pub(crate) mod long_press;
pub(crate) mod model;
pub(crate) mod target;
pub(crate) mod view;

/// AGENTS row height in points (SidebarSpec 5).
pub(crate) const AGENT_ROW_HEIGHT: f32 = 44.;
/// Host row height in points (SidebarSpec 2).
pub(crate) const HOST_ROW_HEIGHT: f32 = 64.;
/// Space row height in points (SidebarSpec 3).
pub(crate) const SPACE_ROW_HEIGHT: f32 = 56.;

/// Sidebar memory that outlives one frame: the saved Hosts order and the
/// order in which this iPad received each agent's status changes.
///
/// The receive order is recorded when an update arrives (`board.rs`
/// `record_board`, called for every host's reader update), not when the list
/// is drawn: a collapsed or hidden sidebar must not reorder AGENTS by
/// snapshot position (review SF-1). `record_board` takes `&self`, so the
/// order sits in a `RefCell`; nothing re-enters it.
#[derive(Debug, Default)]
pub(crate) struct SidebarState {
    /// Saved Hosts order (L7's Hosts list); hosts not in it follow by name.
    pub(crate) host_order: Vec<String>,
    /// The open long-press menu of a host or space row (L6, Q8).
    pub(crate) long_press: Option<long_press::LongPressMenu>,
    order: RefCell<ReceiveOrder>,
}

#[derive(Debug, Default)]
struct ReceiveOrder {
    received: BTreeMap<AgentIdentity, Seen>,
    /// Boot of the batch already taken for each connection.
    batches: BTreeMap<(String, HerdrKind), String>,
    counter: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seen {
    status: SidebarStatus,
    seq: u64,
    stamp: u64,
}

/// One agent as a daemon reported it: pane id (Herdr 2: terminal address),
/// status, and that daemon's own change sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Observed {
    pub(crate) pane: String,
    pub(crate) status: SidebarStatus,
    pub(crate) seq: u64,
}

impl SidebarState {
    /// Records one snapshot of a connection (see `ReceiveOrder::observe`).
    pub(crate) fn observe(&self, host: &str, kind: HerdrKind, boot: &str, agents: &[Observed]) {
        if let Ok(mut order) = self.order.try_borrow_mut() {
            order.observe(host, kind, boot, agents);
        }
    }

    /// Drops a connection's memory; the next snapshot is a first batch.
    pub(crate) fn forget(&self, host: &str, kind: HerdrKind) {
        if let Ok(mut order) = self.order.try_borrow_mut() {
            order.forget(host, kind);
        }
    }

    /// This iPad's receive number for an agent (0 = first batch or unseen).
    pub(crate) fn received(&self, id: &AgentIdentity) -> u64 {
        self.order
            .try_borrow()
            .map_or(0, |order| order.received(id))
    }
}

impl ReceiveOrder {
    /// Records one snapshot of a connection. The first batch after a
    /// connection (or a daemon restart) is stamped 0, so it orders by host
    /// and name; later changes get this iPad's next receive number. A
    /// daemon's `seq` is compared only with the same agent's previous `seq`,
    /// never with another daemon's.
    fn observe(&mut self, host: &str, kind: HerdrKind, boot: &str, agents: &[Observed]) {
        let key = (host.to_owned(), kind);
        let initial = self.batches.get(&key).is_none_or(|seen| seen != boot);
        if initial {
            self.forget(host, kind);
            self.batches.insert(key, boot.to_owned());
        }
        self.received.retain(|id, _| {
            id.host != host || id.kind != kind || agents.iter().any(|a| a.pane == id.pane)
        });
        for agent in agents {
            let id = AgentIdentity {
                host: host.to_owned(),
                kind,
                pane: agent.pane.clone(),
            };
            let changed = self
                .received
                .get(&id)
                .is_none_or(|seen| seen.status != agent.status || seen.seq != agent.seq);
            if !changed {
                continue;
            }
            let stamp = if initial {
                0
            } else {
                self.counter = self.counter.saturating_add(1);
                self.counter
            };
            self.received.insert(
                id,
                Seen {
                    status: agent.status,
                    seq: agent.seq,
                    stamp,
                },
            );
        }
    }

    fn forget(&mut self, host: &str, kind: HerdrKind) {
        self.received
            .retain(|id, _| id.host != host || id.kind != kind);
        self.batches.remove(&(host.to_owned(), kind));
    }

    fn received(&self, id: &AgentIdentity) -> u64 {
        self.received.get(id).map_or(0, |seen| seen.stamp)
    }
}

/// (host, kind, pane id or address), D8.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct AgentIdentity {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) pane: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentRow {
    pub(crate) identity: AgentIdentity,
    pub(crate) name: String,
    pub(crate) tab: u64,
    pub(crate) status: SidebarStatus,
    pub(crate) received: u64,
}

/// AGENTS order: status (Q5), then newest received first; a first batch
/// (equal receive numbers) by host, then name.
pub(crate) fn agent_order(a: &AgentRow, b: &AgentRow) -> Ordering {
    agent_order_in(a, b, &|_| 0)
}

/// `agent_order` with the hosts ranked by `host_rank` (the saved Hosts
/// order) before their names.
pub(crate) fn agent_order_in(
    a: &AgentRow,
    b: &AgentRow,
    host_rank: &dyn Fn(&AgentIdentity) -> usize,
) -> Ordering {
    a.status
        .rank()
        .cmp(&b.status.rank())
        .then_with(|| b.received.cmp(&a.received))
        .then_with(|| host_rank(&a.identity).cmp(&host_rank(&b.identity)))
        .then_with(|| a.identity.host.cmp(&b.identity.host))
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.identity.kind.cmp(&b.identity.kind))
        .then_with(|| a.tab.cmp(&b.tab))
        .then_with(|| a.identity.pane.cmp(&b.identity.pane))
}

/// Q5 order, newest received first within one status.
pub(crate) fn order_agents(rows: &mut [AgentRow]) {
    rows.sort_by(agent_order);
}

/// Shown name: a tab/pane suffix only when names collide. `all` is the
/// rows of the same space; only rows of the same connection can collide.
pub(crate) fn display_name(row: &AgentRow, all: &[AgentRow]) -> String {
    let collides = all.iter().any(|other| {
        other.identity != row.identity
            && other.name == row.name
            && other.identity.host == row.identity.host
            && other.identity.kind == row.identity.kind
    });
    if collides {
        format!(
            "{} · tab {} · pane {}",
            row.name, row.tab, row.identity.pane
        )
    } else {
        row.name.clone()
    }
}

/// Spoken name: always includes tab and pane (S-8).
pub(crate) fn accessibility_name(row: &AgentRow) -> String {
    let kind = match row.identity.kind {
        HerdrKind::Herdr1 => "",
        HerdrKind::Herdr2 => ", Herdr 2",
    };
    format!(
        "{}, {}, tab {}, pane {}, {}{kind}",
        row.name,
        row.status.label(),
        row.tab,
        row.identity.pane,
        row.identity.host
    )
}

/// Row height for a text scale (1.0 = default), S-11. A row never gets
/// shorter than its base touch height.
pub(crate) fn row_height(base: f32, text_scale: f32) -> f32 {
    if text_scale.is_finite() && text_scale > 1. {
        base * text_scale
    } else {
        base
    }
}

#[cfg(test)]
#[path = "sidebar_tests.rs"]
mod tests;
