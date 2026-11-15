//! One paste's lifetime, shared by the UI, the core worker, and its put call.
use sheprd_core_client::{Delivery, WriteGate};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

#[derive(Clone)]
pub(crate) struct PasteTicket {
    pub serial: u64,
    #[cfg(test)]
    pub test: Arc<TestHooks>,
    state: Arc<Mutex<TicketState>>,
}

enum TicketState {
    Pending,
    Queued(WriteGate),
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cancellation {
    Unsent,
    Sent,
    Sending,
}
#[derive(Debug)]
pub(crate) struct Queued;

impl PasteTicket {
    pub fn new(serial: u64) -> Self {
        Self {
            serial,
            #[cfg(test)]
            test: Arc::default(),
            state: Arc::new(Mutex::new(TicketState::Pending)),
        }
    }
    fn lock(&self) -> MutexGuard<'_, TicketState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn pending(&self) -> bool {
        matches!(*self.lock(), TicketState::Pending)
    }
    pub fn fail(&self) {
        let mut state = self.lock();
        if matches!(*state, TicketState::Pending) {
            *state = TicketState::Failed;
        }
    }
    pub fn is_finished(&self) -> bool {
        match &*self.lock() {
            TicketState::Pending => false,
            TicketState::Queued(gate) => gate.is_finished(),
            _ => true,
        }
    }
    pub fn cancel(&self) -> Cancellation {
        let mut state = self.lock();
        match &*state {
            TicketState::Pending => {
                *state = TicketState::Cancelled;
                Cancellation::Unsent
            }
            TicketState::Queued(gate) => match gate.cancel() {
                Delivery::Written => Cancellation::Sent,
                Delivery::Writing => Cancellation::Sending,
                _ => Cancellation::Unsent,
            },
            _ => Cancellation::Unsent,
        }
    }
    pub fn delivery(&self) -> Option<Delivery> {
        match &*self.lock() {
            TicketState::Queued(gate) => Some(gate.state()),
            _ => None,
        }
    }
    pub fn queue(
        &self,
        send: impl FnOnce() -> Result<Option<WriteGate>, String>,
    ) -> Result<Queued, String> {
        let mut state = self.lock();
        if !matches!(*state, TicketState::Pending) {
            return Err("Paste cancelled: target changed".into());
        }
        match send() {
            Ok(Some(gate)) => {
                *state = TicketState::Queued(gate);
                Ok(Queued)
            }
            Ok(None) => {
                *state = TicketState::Failed;
                Err("Paste failed: terminal ended".into())
            }
            Err(error) => {
                *state = TicketState::Failed;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestHooks {
    pub timed_out: std::sync::atomic::AtomicBool,
    pub put_busy: std::sync::atomic::AtomicBool,
    pub input_busy: std::sync::atomic::AtomicBool,
    pub before_input: crate::core_mock::transport::Latch,
    pub notification: crate::core_mock::transport::Latch,
    pub ui_completion: crate::core_mock::transport::Latch,
}
