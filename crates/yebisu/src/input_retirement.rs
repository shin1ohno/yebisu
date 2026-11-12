//! Consumer ownership correlation. This does not establish OS input cancellation.
use crate::{app::Smoke, terminal_input::Destination};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Owner {
    host: String,
    generation: u64,
    selection: u64,
    revision: u64,
    destination: Option<Destination>,
    open: bool,
    suspended: bool,
    core: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ticket {
    pub serial: u64,
    owner: Owner,
    retired: u64,
}

#[derive(Default)]
pub(crate) struct Retirement {
    depth: usize,
    serial: u64,
    pending: Option<Ticket>,
    pub reason: Option<&'static str>,
    #[cfg(target_os = "ios")]
    pub native: Option<crate::native_retirement::Binding>,
    #[cfg(target_os = "ios")]
    pub receipt: Option<herdr_uikit::TextInputRetirement>,
}

impl Smoke {
    fn retirement_owner(&self) -> Owner {
        Owner {
            host: self.terminal.reconnect.target.clone(),
            generation: self.terminal.link.generation,
            selection: self.board.selection,
            revision: self.terminal.input.discard_revision(),
            destination: self.terminal.input.destination.clone(),
            open: self.terminal.open,
            suspended: self.terminal.reconnect.suspended,
            core: self.terminal.core_mode,
        }
    }

    /// Batch nested host/connect operations into one request for their final owner.
    /// No timer retries: a refused native entry stays fenced until another explicit
    /// transition. In particular, native key/text callbacks cannot be recovered by
    /// scheduling their input for later delivery.
    pub(crate) fn input_transition<R>(&mut self, change: impl FnOnce(&mut Self) -> R) -> R {
        self.retirement.depth += 1;
        let result = change(self);
        self.retirement.depth -= 1;
        if self.retirement.depth == 0 {
            self.request_text_retirement();
        }
        result
    }

    #[cfg(not(target_os = "ios"))]
    fn request_text_retirement(&mut self) {
        if self.terminal.input.requires_gesture() {
            self.retirement.reason = Some("Text input paused · native retirement unavailable");
        }
    }

    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(crate) fn begin_text_retirement(&mut self, retired: u64) -> Option<Ticket> {
        if !self.terminal.input.requires_gesture()
            || !self.terminal.open
            || self.terminal.reconnect.suspended
            || (!self.terminal.live.connected && !self.terminal.needs_focus)
        {
            return None;
        }
        let Some(serial) = self.retirement.serial.checked_add(1) else {
            self.retirement.reason = Some("Text input paused · retirement identity exhausted");
            return None;
        };
        let ticket = Ticket {
            serial,
            owner: self.retirement_owner(),
            retired,
        };
        self.retirement.serial = serial;
        self.retirement.pending = Some(ticket.clone());
        self.retirement.reason = Some("Text input paused · retiring previous composition");
        Some(ticket)
    }

    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(crate) fn complete_text_retirement(
        &mut self,
        ticket: &Ticket,
        retired: u64,
        replacement: u64,
    ) -> bool {
        if self.retirement.pending.as_ref() != Some(ticket) {
            return false;
        }
        self.retirement.pending = None;
        if ticket.owner != self.retirement_owner()
            || ticket.retired != retired
            || retired.checked_add(1) != Some(replacement)
        {
            self.retirement.reason = Some("Text input paused · retirement owner changed");
            return false;
        }
        let accepted = self
            .terminal
            .input
            .acknowledge_retirement(ticket.owner.revision);
        if accepted {
            self.retirement.reason = None;
        }
        accepted
    }

    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(crate) fn refuse_text_retirement(&mut self, ticket: &Ticket, reason: &'static str) {
        if self.retirement.serial == ticket.serial {
            self.retirement.pending = None;
            self.retirement.reason = Some(reason);
        }
    }

    /// Host tests supply the native boundary explicitly. This is never native or
    /// OS proof, and is deliberately absent from application/Simulator builds.
    #[cfg(test)]
    pub(crate) fn controlled_text_retirement(&mut self) {
        let ticket = self
            .begin_text_retirement(40)
            .unwrap_or_else(|| panic!("discarded active owner"));
        assert!(self.complete_text_retirement(&ticket, 40, 41));
    }
}

#[cfg(test)]
#[path = "input_retirement_tests.rs"]
mod tests;
