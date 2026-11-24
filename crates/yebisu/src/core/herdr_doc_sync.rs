//! The `herdr-app` document of a `default` connection (issue #144), read on
//! a thread of its own like the status poll (`core/status.rs`).
//!
//! A core call can wait for herdr-core's call timeout (10 s), so neither the
//! UI thread nor the worker loop reads the document. This thread reads it
//! once at the start, again on every `doc` event for `herdr-app` newer than
//! the version it holds and on every `missed`, and at least every
//! `DOC_POLL` whatever the event stream does; a stream that ended is opened
//! again at the next poll. With each read it lists the live terminals (a
//! document block whose terminal is gone is not drawn). The result goes into
//! a latest-state mailbox the session owns; a change marks the shared `Live`
//! dirty, and the worker, the only publisher of the session's rows, takes it.
use super::herdr_doc::{self, View};
use crate::live::Live;
use sheprd_core_client::{CoreBackend, event::Notice, wire::TerminalState};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// The longest the document goes unread, events or not (Design B-2, AC3).
pub(crate) const DOC_POLL: Duration = Duration::from_secs(5);
/// How long one wait for events lasts before the stop flag is checked.
const EVENT_WAIT: Duration = Duration::from_millis(250);
/// How long a new connection waits for its first read: herdr-core's own
/// call timeout.
const FIRST_READ: Duration = Duration::from_secs(10);
/// How often a thread without a stream checks that it should stop.
const STOP_CHECK: Duration = Duration::from_millis(50);

/// What the last read found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Read {
    Readable(View),
    /// Not the official app's shape: shown as such, never written (B-4).
    Unreadable,
    /// The read failed; the text is the client's error.
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DocSnapshot {
    pub(crate) version: u64,
    pub(crate) read: Read,
    /// The live terminals listed with the read.
    pub(crate) live: Vec<String>,
    /// Automatic space names by folder (owner decision 9).
    pub(crate) names: BTreeMap<String, String>,
}

/// The session's latest document read. The count grows with each change,
/// so the worker takes each one once.
#[derive(Clone, Default)]
pub(crate) struct DocMailbox(Arc<Mutex<(u64, Option<DocSnapshot>)>>);

impl DocMailbox {
    /// Stores `next`; true when it differs from what was there.
    pub(crate) fn publish(&self, next: DocSnapshot) -> bool {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.1.as_ref() == Some(&next) {
            return false;
        }
        slot.0 += 1;
        slot.1 = Some(next);
        true
    }

    /// The first read, so a new connection draws the right space at once;
    /// `None` when the session stopped or the read took longer than
    /// `FIRST_READ` (the session then draws its iPad-only tabs and takes
    /// the read when it comes).
    pub(crate) fn first(&self, seen: &mut u64, stop: &AtomicBool) -> Option<DocSnapshot> {
        let deadline = Instant::now() + FIRST_READ;
        while !stop.load(Ordering::Acquire) && Instant::now() < deadline {
            if let Some(snapshot) = self.since(seen) {
                return Some(snapshot);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    /// The snapshot published after `seen`, which moves past it.
    pub(crate) fn since(&self, seen: &mut u64) -> Option<DocSnapshot> {
        let slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.0 == *seen {
            return None;
        }
        *seen = slot.0;
        slot.1.clone()
    }
}

/// Where a session's document comes from.
pub(crate) enum DocSource {
    /// `doc.get` and `events.subscribe` on the session's connection.
    Core,
    /// No reads: transcript-exact tests keep their wire traffic as written.
    #[cfg(test)]
    Off,
}

/// The reads a `default` session runs: from where and how often at least.
pub(crate) struct DocPlan {
    pub(crate) source: DocSource,
    pub(crate) period: Duration,
}

impl Default for DocPlan {
    /// The application reads. Test builds start sessions without reads
    /// unless a test asks (like `status::PollPlan`).
    fn default() -> Self {
        Self {
            #[cfg(not(test))]
            source: DocSource::Core,
            #[cfg(test)]
            source: DocSource::Off,
            period: DOC_POLL,
        }
    }
}

/// The running reader. Dropping it stops the thread and joins it within
/// `status::JOIN_BOUND`.
pub(crate) struct DocSync {
    stop: Arc<AtomicBool>,
    ended: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl DocSync {
    /// The reader for `host`'s `default` core, or `None` for a plan
    /// without reads.
    pub(crate) fn start(
        plan: DocPlan,
        (backend, host): (&CoreBackend, &str),
        mailbox: DocMailbox,
        inbox: Arc<Mutex<Live>>,
        session_stop: Arc<AtomicBool>,
    ) -> std::io::Result<Option<Self>> {
        match plan.source {
            DocSource::Core => {}
            #[cfg(test)]
            DocSource::Off => return Ok(None),
        }
        let mut reader = Reader {
            backend: backend.clone(),
            host: host.to_owned(),
            mailbox,
            inbox,
            held: None,
            live: Vec::new(),
            names: BTreeMap::new(),
            asked: None,
            period: plan.period,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let own = stop.clone();
        let (ended_tx, ended) = mpsc::channel::<()>();
        let stopped = move || own.load(Ordering::Acquire) || session_stop.load(Ordering::Acquire);
        let thread = std::thread::Builder::new()
            .name("yebisu-core-doc".into())
            .spawn(move || {
                // Dropped when this thread ends: the guard's bounded wait.
                let _ended = ended_tx;
                let mut events = None;
                let mut last_read: Option<Instant> = None;
                while !stopped() {
                    let mut due = last_read.is_none_or(|at| at.elapsed() >= reader.period);
                    if due && events.is_none() {
                        events = reader.backend.core().subscribe().ok();
                    }
                    match events.as_mut() {
                        Some(subscription) => match subscription.next(EVENT_WAIT) {
                            Ok(notices) => due |= notices.iter().any(|n| reader.news(n)),
                            // Polled from now on; opened again at the next poll.
                            Err(_) => events = None,
                        },
                        None => wait(EVENT_WAIT, &stopped),
                    }
                    if due && !stopped() {
                        reader.read(&stopped);
                        last_read = Some(Instant::now());
                    }
                }
            })?;
        Ok(Some(Self {
            stop,
            ended,
            thread: Some(thread),
        }))
    }
}

impl Drop for DocSync {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        match self.ended.recv_timeout(super::status::JOIN_BOUND) {
            // Nothing is ever sent: the thread ended and dropped its sender.
            Err(mpsc::RecvTimeoutError::Disconnected) | Ok(()) => {
                if let Some(thread) = self.thread.take()
                    && thread.join().is_err()
                {
                    tracing::warn!("Herdr 2 document reader panicked");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                tracing::warn!("Herdr 2 document read still waiting; left to end on its own");
            }
        }
    }
}

struct Reader {
    backend: CoreBackend,
    #[cfg_attr(not(all(target_os = "ios", not(test))), allow(dead_code))]
    host: String,
    mailbox: DocMailbox,
    inbox: Arc<Mutex<Live>>,
    /// The version of the last read.
    held: Option<u64>,
    live: Vec<String>,
    names: BTreeMap<String, String>,
    /// The folders last asked about, and when.
    #[cfg_attr(not(all(target_os = "ios", not(test))), allow(dead_code))]
    asked: Option<(Vec<String>, Instant)>,
    period: Duration,
}

impl Reader {
    /// Whether `notice` means the document may have changed.
    fn news(&self, notice: &Notice) -> bool {
        match notice {
            Notice::Doc {
                name,
                machine: None,
                version,
                ..
            } => name == herdr_doc::NAME && self.held.is_none_or(|held| *version > held),
            Notice::Missed { .. } => true,
            _ => false,
        }
    }

    fn read(&mut self, stopped: &impl Fn() -> bool) {
        let core = self.backend.core();
        let read = match core.doc_get(herdr_doc::NAME) {
            Ok(doc) => {
                self.held = Some(doc.version);
                match herdr_doc::parse_view(&doc.value) {
                    Ok(view) => Read::Readable(view),
                    Err(error) => {
                        tracing::warn!(%error, "Herdr 2 document");
                        Read::Unreadable
                    }
                }
            }
            Err(error) => Read::Unavailable(error.to_string()),
        };
        // A failed list keeps the last one: a pane is not dropped for it.
        if let Ok(list) = core.list(false) {
            self.live = list
                .terminals
                .into_iter()
                .filter(|terminal| terminal.state == TerminalState::Live)
                .map(|terminal| terminal.address)
                .collect();
        }
        if let Read::Readable(view) = &read {
            self.name_spaces(view, stopped);
        }
        let snapshot = DocSnapshot {
            version: self.held.unwrap_or_default(),
            read,
            live: self.live.clone(),
            names: self.names.clone(),
        };
        if self.mailbox.publish(snapshot)
            && let Ok(mut live) = self.inbox.lock()
        {
            live.dirty = true;
        }
    }

    /// Asks the host for the automatic names when the folders of unnamed
    /// spaces changed, at most once per poll period. Never in tests or in
    /// the Simulator QA mode, which have no host to ask.
    #[cfg_attr(not(all(target_os = "ios", not(test))), allow(unused_variables))]
    fn name_spaces(&mut self, view: &View, stopped: &impl Fn() -> bool) {
        #[cfg(all(target_os = "ios", not(test)))]
        {
            if cfg!(target_abi = "sim") && std::env::var_os("YEBISU_CORE_QA").is_some() {
                return;
            }
            let mut folders = view
                .spaces
                .iter()
                .filter(|space| space.name.trim().is_empty() && !space.folder.is_empty())
                .map(|space| space.folder.clone())
                .collect::<Vec<_>>();
            folders.sort();
            folders.dedup();
            let fresh = self
                .asked
                .as_ref()
                .is_some_and(|(asked, at)| *asked == folders || at.elapsed() < self.period);
            if folders.is_empty() || fresh {
                return;
            }
            self.asked = Some((folders.clone(), Instant::now()));
            if let Some(names) =
                crate::shell::herdr2_frame::names::ask(&self.host, &folders, stopped)
            {
                self.names = names;
            }
        }
    }
}

fn wait(period: Duration, stopped: &impl Fn() -> bool) {
    let mut waited = Duration::ZERO;
    while waited < period && !stopped() {
        let step = STOP_CHECK.min(period - waited);
        std::thread::sleep(step);
        waited += step;
    }
}

#[cfg(test)]
#[path = "herdr_doc_sync_tests.rs"]
mod tests;
