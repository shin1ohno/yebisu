//! Herdr 2 terminal statuses for the sidebar (plan v2 7.14), read by a
//! poll on a thread of its own.
//!
//! The worker loop that sends input and Resize never lists: `core.list`
//! waits for its answer (herdr-core's call timeout, 10 s), and that wait
//! stays on the `yebisu-core-status` thread. The result goes into a
//! latest-state mailbox the session owns; a change marks the shared `Live`
//! dirty so the UI draws again, and the UI reads the list from the session.
//! The poll stops when its guard drops (the worker returned) or the session
//! is dropped, and the guard joins it within `JOIN_BOUND`.
use crate::{live::Live, shell::herdr2_frame::CoreStatus};
use std::{
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

/// How often the statuses are listed.
pub(crate) const STATUS_POLL: Duration = Duration::from_secs(1);
/// How often a waiting poll checks that it should stop.
const STOP_CHECK: Duration = Duration::from_millis(50);
/// How long dropping the guard waits for the poll thread to end. A list in
/// flight when the session ends returns once the worker's disconnect fails
/// it; one that does not within this bound is left to end on its own
/// (bounded by herdr-core's call timeout) so the session's thread finishes.
pub(crate) const JOIN_BOUND: Duration = Duration::from_millis(500);

/// One listing: every terminal's raw status, or `None` when it failed.
pub(crate) type List = Box<dyn FnMut() -> Option<Vec<CoreStatus>> + Send>;

/// Where a session's statuses come from.
pub(crate) enum StatusSource {
    /// `core.list` on the session's own connection (the application).
    Core,
    /// No poll: transport-gated tests keep their wire traffic as written.
    #[cfg(test)]
    Off,
    /// A test's own listing.
    #[cfg(test)]
    Custom(List),
}

/// The poll a session runs: what it lists and how often.
pub(crate) struct PollPlan {
    pub(crate) source: StatusSource,
    pub(crate) period: Duration,
}

impl Default for PollPlan {
    /// The application lists every second. Test builds start sessions with
    /// no poll unless a test asks for one (review e85e B1).
    fn default() -> Self {
        Self {
            #[cfg(not(test))]
            source: StatusSource::Core,
            #[cfg(test)]
            source: StatusSource::Off,
            period: STATUS_POLL,
        }
    }
}

impl PollPlan {
    /// The listing for `backend`, or `None` when this plan has no poll.
    pub(super) fn listing(self, backend: &sheprd_core_client::CoreBackend) -> Option<List> {
        match self.source {
            StatusSource::Core => {
                let backend = backend.clone();
                Some(Box::new(move || {
                    // A failed list only skips this poll; the worker's own
                    // reads end the connection when the link is gone.
                    backend.core().list(false).ok().map(|list| {
                        list.terminals
                            .iter()
                            .map(crate::shell::herdr2_frame::core_status)
                            .collect()
                    })
                }))
            }
            #[cfg(test)]
            StatusSource::Off => None,
            #[cfg(test)]
            StatusSource::Custom(list) => Some(list),
        }
    }
}

/// The latest statuses a session listed. `None` until the first list
/// answers: the sidebar draws no Herdr 2 list without one.
#[derive(Clone, Default)]
pub(crate) struct StatusMailbox(Arc<Mutex<Option<Vec<CoreStatus>>>>);

impl StatusMailbox {
    pub(crate) fn latest(&self) -> Option<Vec<CoreStatus>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Stores `next`; true when it differs from what was there.
    pub(crate) fn publish(&self, next: Vec<CoreStatus>) -> bool {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref() == Some(&next) {
            return false;
        }
        *slot = Some(next);
        true
    }
}

/// The running poll. Dropping it stops the thread and joins it within
/// `JOIN_BOUND`.
pub(crate) struct StatusPoll {
    stop: Arc<AtomicBool>,
    ended: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl StatusPoll {
    /// Lists every `period` until the guard drops or `session_stop` is set.
    /// A change goes into `mailbox` and marks `inbox` dirty.
    pub(crate) fn start(
        mut list: List,
        mailbox: StatusMailbox,
        inbox: Arc<Mutex<Live>>,
        session_stop: Arc<AtomicBool>,
        period: Duration,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let own = stop.clone();
        let (ended_tx, ended) = mpsc::channel::<()>();
        let stopped = move || own.load(Ordering::Acquire) || session_stop.load(Ordering::Acquire);
        let thread = std::thread::Builder::new()
            .name("yebisu-core-status".into())
            .spawn(move || {
                // Dropped when this thread ends: the guard's bounded wait.
                let _ended = ended_tx;
                while !stopped() {
                    if let Some(next) = list()
                        && !stopped()
                        && mailbox.publish(next)
                        && let Ok(mut live) = inbox.lock()
                    {
                        live.dirty = true;
                    }
                    let mut waited = Duration::ZERO;
                    while waited < period && !stopped() {
                        let step = STOP_CHECK.min(period - waited);
                        std::thread::sleep(step);
                        waited += step;
                    }
                }
            })?;
        Ok(Self {
            stop,
            ended,
            thread: Some(thread),
        })
    }
}

impl Drop for StatusPoll {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        match self.ended.recv_timeout(JOIN_BOUND) {
            // Nothing is ever sent: the thread ended and dropped its sender.
            Err(mpsc::RecvTimeoutError::Disconnected) | Ok(()) => {
                if let Some(thread) = self.thread.take()
                    && thread.join().is_err()
                {
                    tracing::warn!("Herdr 2 status poll panicked");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                tracing::warn!("Herdr 2 status poll still listing; left to end on its own");
            }
        }
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
