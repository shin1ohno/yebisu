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
/// Failed lists in a row after which the last statuses are dropped rather
/// than shown as current (review 0df6 SF3). Each failure can take the call
/// timeout (10 s), so one failure alone is not taken as the end of a list.
pub(crate) const STALE_AFTER: u32 = 2;

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
    /// The application's listing also keeps in `mailbox` which live
    /// terminals run a program or an agent, for Close Space (#249).
    pub(super) fn listing(
        self,
        backend: &sheprd_core_client::CoreBackend,
        mailbox: &StatusMailbox,
    ) -> Option<List> {
        match self.source {
            StatusSource::Core => {
                let backend = backend.clone();
                let busy = mailbox.busy.clone();
                Some(Box::new(move || {
                    // A failed list only skips this poll; the worker's own
                    // reads end the connection when the link is gone.
                    let list = backend.core().list(false).ok()?;
                    *busy.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some(super::running::busy(&list.terminals));
                    Some(list.terminals.iter().map(CoreStatus::from).collect())
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
pub(crate) struct StatusMailbox {
    statuses: Arc<Mutex<Option<Vec<CoreStatus>>>>,
    /// The live terminals of the same list that run a program or an agent
    /// (`running::busy`); `None` until a list answered.
    busy: Arc<Mutex<Option<Vec<String>>>>,
}

impl StatusMailbox {
    pub(crate) fn latest(&self) -> Option<Vec<CoreStatus>> {
        self.statuses
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The busy terminals of the last list, or `None` when none is known.
    pub(crate) fn busy(&self) -> Option<Vec<String>> {
        self.busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn publish_busy(&self, busy: Vec<String>) {
        *self.busy.lock().unwrap_or_else(PoisonError::into_inner) = Some(busy);
    }

    /// Forgets the statuses (the list keeps failing); true when there were some.
    pub(crate) fn clear(&self) -> bool {
        self.busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.statuses
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .is_some()
    }

    /// Stores `next`; true when it differs from what was there.
    pub(crate) fn publish(&self, next: Vec<CoreStatus>) -> bool {
        let mut slot = self.statuses.lock().unwrap_or_else(PoisonError::into_inner);
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
    /// A change goes into `mailbox` and marks `inbox` dirty; `STALE_AFTER`
    /// failed lists in a row empty the mailbox, so the sidebar stops showing
    /// statuses nobody can confirm.
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
                let mut failures = 0;
                while !stopped() {
                    let changed = match list() {
                        Some(next) => {
                            failures = 0;
                            !stopped() && mailbox.publish(next)
                        }
                        None => {
                            failures += 1;
                            failures >= STALE_AFTER && !stopped() && mailbox.clear()
                        }
                    };
                    if changed && let Ok(mut live) = inbox.lock() {
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
#[path = "status_stale_tests.rs"]
mod stale_tests;
#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
