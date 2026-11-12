//! One app-local key owner. No key or provider error implements diagnostic Debug.
mod provider;
use crate::settings_pages::device_key::KeyView;
pub(crate) use provider::{Failure, Provider};
use std::{
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ticket {
    owner: u64,
    view: u64,
    key: u64,
    sequence: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Unavailable,
    Ready,
    Confirming,
    Reading,
    Submitted,
    Unknown,
    NotStarted,
    Unsupported,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Read,
    Replace,
}
struct Reply {
    op: u64,
    key: u64,
    value: Result<String, Failure>,
}
struct Worker {
    handle: JoinHandle<()>,
    receiver: mpsc::Receiver<Reply>,
    op: u64,
    key: u64,
    kind: Operation,
    started: Instant,
    admitted: bool,
}
pub(crate) struct Owner {
    provider: Arc<dyn Provider>,
    identity: u64,
    pub state: State,
    pub failure: Option<Failure>,
    view: u64,
    key: u64,
    sequence: u64,
    ticket: Option<Ticket>,
    display: KeyView,
    worker: Option<Worker>,
    unresolved: bool,
}
impl Default for Owner {
    fn default() -> Self {
        Self::new(provider::platform())
    }
}
impl Owner {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let state = if provider.supported() && provider.identity() == provider::IDENTITY {
            State::Unavailable
        } else {
            State::Unsupported
        };
        Self {
            provider,
            identity: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            state,
            failure: None,
            view: 0,
            key: 0,
            sequence: 0,
            ticket: None,
            display: KeyView::failed(),
            worker: None,
            unresolved: false,
        }
    }
    pub fn supported(&self) -> bool {
        self.provider.supported() && self.provider.identity() == provider::IDENTITY
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn line(&self) -> Option<&str> {
        self.display.copy_text(self.busy() || self.unresolved)
    }
    pub fn view_open(&mut self) {
        self.view = self.view.wrapping_add(1);
        self.cancel();
    }
    pub fn cancel(&mut self) {
        self.ticket = None;
        if self.state == State::Confirming {
            self.state = State::Ready;
        }
    }
    pub fn confirmation(&mut self) -> Option<Ticket> {
        if self.busy() || self.unresolved || self.line().is_none() {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let ticket = Ticket {
            owner: self.identity,
            view: self.view,
            key: self.key,
            sequence: self.sequence,
        };
        self.ticket = Some(ticket);
        self.state = State::Confirming;
        Some(ticket)
    }
    pub fn confirm(&mut self, ticket: Ticket, visible: bool, repeated: bool) -> bool {
        if !visible || repeated || self.busy() || self.ticket != Some(ticket) || self.unresolved {
            return false;
        }
        self.ticket = None;
        self.start(Operation::Replace)
    }
    pub fn read_or_initialize(&mut self) -> bool {
        if self.busy() || !self.supported() {
            return false;
        }
        self.cancel();
        self.start(Operation::Read)
    }
    fn start(&mut self, kind: Operation) -> bool {
        let (send, receive) = mpsc::sync_channel::<Operation>(1);
        let (done, receiver) = mpsc::sync_channel(1);
        self.sequence = self.sequence.wrapping_add(1);
        let op = self.sequence;
        let key = self.key.wrapping_add(1);
        let provider = self.provider.clone();
        // A failed spawn/send has not invoked the provider. Never retry implicitly.
        let handle = match std::thread::Builder::new()
            .name("device-key".into())
            .spawn(move || {
                if let Ok(request) = receive.recv() {
                    let value = match request {
                        Operation::Read => provider.read_or_initialize(),
                        Operation::Replace => provider.replace(),
                    }
                    .and_then(|raw| {
                        crate::keys::authorized_line(&raw).map_err(|_| Failure::Format)
                    });
                    let _ = done.send(Reply { op, key, value });
                }
            }) {
            Ok(handle) => handle,
            Err(_) => {
                self.state = State::NotStarted;
                self.failure = Some(Failure::Worker);
                return false;
            }
        };
        let submitted = self.enqueue(&send, kind, key);
        self.worker = Some(Worker {
            handle,
            receiver,
            op,
            key,
            kind,
            started: Instant::now(),
            admitted: submitted,
        });
        submitted
    }
    fn enqueue(&mut self, send: &mpsc::SyncSender<Operation>, kind: Operation, key: u64) -> bool {
        let submitted = send.try_send(kind).is_ok();
        // These mutations run in the same UI update as successful enqueue.
        if submitted {
            self.key = key;
            self.display = KeyView::loading();
            self.failure = None;
            if kind == Operation::Replace {
                self.unresolved = true;
            }
            self.state = if kind == Operation::Replace {
                State::Submitted
            } else {
                State::Reading
            };
        } else {
            self.state = State::NotStarted;
            self.failure = Some(Failure::Queue);
        }
        submitted
    }
    pub fn poll(&mut self, now: Instant) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        if !worker.handle.is_finished() {
            if now.saturating_duration_since(worker.started) >= Duration::from_secs(15)
                && self.state != State::Unknown
            {
                self.state = State::Unknown;
                self.failure = Some(Failure::Timeout);
                self.display = KeyView::failed();
                return true;
            }
            return false;
        }
        let Some(worker) = self.worker.take() else {
            return false;
        };
        let joined = worker.handle.join().is_ok();
        if !worker.admitted {
            return true;
        }
        let reply = worker
            .receiver
            .try_recv()
            .ok()
            .filter(|r| r.op == worker.op && r.key == worker.key && r.key == self.key);
        match (joined, worker.kind, reply.map(|r| r.value)) {
            (true, Operation::Read, Some(Ok(line))) => {
                self.display = KeyView::from_formatted_worker_line(line);
                self.unresolved = false;
                self.state = State::Ready;
                self.failure = None;
            }
            (true, Operation::Replace, Some(Ok(_))) => {
                // Even successful mutation needs an explicit current-key reconciliation.
                self.display = KeyView::failed();
                self.state = State::Unknown;
                self.failure = None;
            }
            (_, _, result) => {
                self.display = KeyView::failed();
                self.state = if self.unresolved {
                    State::Unknown
                } else {
                    State::Unavailable
                };
                self.failure = Some(result.and_then(Result::err).unwrap_or(Failure::Worker));
            }
        }
        true
    }
    pub fn label(&self) -> &'static str {
        match self.state {
            State::Unavailable => "No current key. Read or initialize explicitly.",
            State::Unsupported => "Device keys are unavailable on this platform.",
            State::Ready => "Current public key",
            State::Confirming => {
                "Replace this installation's key? Every computer needs the new key registered manually. Existing sessions stay connected."
            }
            State::Reading => "Reading or initializing device key…",
            State::Submitted => "Replacing device key…",
            State::Unknown => {
                "Outcome needs reconciliation. Wait for the worker to finish, then read or initialize the current key. Do not replace again."
            }
            State::NotStarted => "Operation did not start. No automatic retry.",
        }
    }
}
#[cfg(test)]
mod tests;
