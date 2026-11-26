//! Taps and keys for the runner, as event target IDs, in tap order.
use super::{Result, Scene};
use std::collections::VecDeque;
use yebisu_mod_wire as wire;

/// Events held for the line. The UI thread never waits on the line: when the
/// queue is full a new event is dropped and counted, never blocked on.
pub(crate) const EVENT_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    Click,
    Key,
}

/// One routed event. `target` is the wire node index in view `generation` of
/// publication `pub_rev`; `callback` is the callback ID the node carried.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModEvent {
    pub(crate) generation: u64,
    pub(crate) pub_rev: u64,
    pub(crate) target: usize,
    pub(crate) callback: u64,
    pub(crate) kind: EventKind,
    pub(crate) key: Option<String>,
}

impl ModEvent {
    pub(crate) fn new(
        scene: &Scene,
        target: usize,
        callback: u64,
        kind: EventKind,
        key: Option<String>,
    ) -> Self {
        Self {
            generation: scene.generation,
            pub_rev: scene.pub_rev,
            target,
            callback,
            kind,
            key,
        }
    }

    /// The wire `Action` the line would carry, with its body digest. The
    /// operation sequence, event ID and display lease come from the session.
    pub(crate) fn to_action(
        &self,
        op_seq: u64,
        event_id: String,
        lease_id: String,
    ) -> Result<wire::Action> {
        let mut action = wire::Action {
            op_seq: wire::Counter(op_seq),
            event_id,
            after_local_seq: wire::Counter(0),
            pub_rev: wire::Counter(self.pub_rev),
            generation: wire::Counter(self.generation),
            lease_id,
            target: self.target,
            kind: match self.kind {
                EventKind::Click => wire::ActionKind::Click,
                EventKind::Key => wire::ActionKind::Key,
            },
            data: wire::ActionData {
                key: self.key.clone(),
                text: None,
                point: None,
                value: None,
                row_key: None,
                permissions: Vec::new(),
            },
            digest: wire::bytes_digest(b""),
        };
        action.digest = wire::canonical_digest(&action, "digest")?;
        action.validate()?;
        Ok(action)
    }
}

/// Bounded FIFO of routed events.
#[derive(Debug, Default)]
pub(crate) struct EventQueue {
    events: VecDeque<ModEvent>,
    dropped: u64,
}

impl EventQueue {
    /// Queues `event` unless the queue is full. Returns whether it was queued.
    pub(crate) fn push(&mut self, event: ModEvent) -> bool {
        if self.events.len() >= EVENT_CAPACITY {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.events.push_back(event);
        true
    }

    /// Everything queued, oldest first.
    pub(crate) fn drain(&mut self) -> Vec<ModEvent> {
        self.events.drain(..).collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }
}
