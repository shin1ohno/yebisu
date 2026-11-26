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
//!
//! The same thread writes (#144 D4): the worker queues intents, and this
//! thread applies each to the latest document, checks it (D-4) and sends it
//! over the version it read, one at a time (D-6). A stale version is read
//! again and the intent applied to the newer document, at most `MAX_SENDS`
//! times (owner decision 5); the previous value is never sent again under a
//! newer version. Every write is read back and checked (R1).
//!
//! The same thread takes ended terminals' panes out, with the tabs and
//! spaces that leaves empty (`ended_spaces`), from its own complete reads
//! only, through the same intent path and checks.
use super::{
    ended_spaces::{self, Listing, Tracker},
    herdr_doc::{self, Applied, Intent, View, WriteCheck},
};
use crate::live::Live;
use serde_json::Value;
use sheprd_core_client::{CoreBackend, Error as CoreError, event::Notice, wire::TerminalState};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
/// Intents waiting for the reader; the worker never waits for a write.
const WRITE_QUEUE: usize = 16;
/// At most this many `doc.set`s per intent (owner decision 5).
pub(crate) const MAX_SENDS: usize = 3;
/// The notices of an intent that was not saved.
pub(crate) const CHANGED_ELSEWHERE: &str = "Changed elsewhere; the iPad change was not saved";
const REFUSED: &str = "The herdr-app layout change failed a check and was not sent";
const NOT_READABLE: &str = "Can't read the herdr-app layout; the iPad change was not sent";

/// Every reader so far: one per connection, so one per herdr-core boot.
static READERS: AtomicU64 = AtomicU64::new(0);

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
    pub(crate) stats: WriteStats,
}

/// What became of the intents so far (`qa.state.herdr_doc`, the status).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct WriteStats {
    /// `doc.set`s herdr-core accepted.
    pub(crate) writes: u64,
    /// `doc.set`s refused as stale.
    pub(crate) conflicts: u64,
    /// The last intent: `saved`, `unchanged`, `conflict-lost`,
    /// `check-failed:<gate>`, `not-readable` or `unknown`.
    pub(crate) last: Option<String>,
    /// What the user is told about the last intent, when it was not saved.
    pub(crate) notice: Option<String>,
    /// Intents finished, so a notice shows again for the next one.
    pub(crate) finished: u64,
    /// Spaces whose ended panes (and tabs, or the whole space) were taken
    /// out of the document.
    pub(crate) ended: u64,
}

/// The worker's end of the write queue.
#[derive(Clone)]
pub(crate) struct DocWriter(mpsc::SyncSender<Intent>);

impl DocWriter {
    /// Queues `intent`; `Err` when the queue is full or the reader ended.
    pub(crate) fn send(&self, intent: Intent) -> crate::Result<()> {
        self.0.try_send(intent).map_err(|_| crate::Error::CoreQueue)
    }
}

/// Intents whose send the connection lost, by host (D-7). The next reader
/// of that host applies each once more after its first read: an intent
/// that did land is then `Unchanged`, so nothing is sent twice.
static UNSENT: Mutex<BTreeMap<String, Intent>> = Mutex::new(BTreeMap::new());

fn unsent() -> std::sync::MutexGuard<'static, BTreeMap<String, Intent>> {
    UNSENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How one intent ended.
enum Outcome {
    Unchanged,
    Saved,
    Lost,
    Refused(WriteCheck),
    NotReadable,
    Unknown,
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
    /// The reader (and writer) for `host`'s `default` core, or `None` for
    /// a plan without reads.
    pub(crate) fn start(
        plan: DocPlan,
        (backend, host): (&CoreBackend, &str),
        mailbox: DocMailbox,
        inbox: Arc<Mutex<Live>>,
        session_stop: Arc<AtomicBool>,
    ) -> std::io::Result<Option<(Self, DocWriter)>> {
        match plan.source {
            DocSource::Core => {}
            #[cfg(test)]
            DocSource::Off => return Ok(None),
        }
        let (writer, writes) = mpsc::sync_channel(WRITE_QUEUE);
        let boot = format!(
            "{}#{}",
            backend.core().hello().core.id,
            READERS.fetch_add(1, Ordering::Relaxed)
        );
        let mut reader = Reader {
            backend: backend.clone(),
            host: host.to_owned(),
            mailbox,
            inbox,
            held: None,
            doc: None,
            live: Vec::new(),
            names: BTreeMap::new(),
            asked: None,
            period: plan.period,
            stats: WriteStats::default(),
            reported: None,
            boot,
            tracker: Tracker::new(plan.period),
            due: Vec::new(),
        };
        let mut unsent = unsent().remove(host);
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
                        reader.remove_ended(&stopped);
                    }
                    // One intent at a time, each applied to the latest read.
                    if last_read.is_some() && !stopped() {
                        let next = unsent.take().or_else(|| writes.try_recv().ok());
                        if let Some(intent) = next {
                            reader.write(intent, &stopped);
                        }
                    }
                }
            })?;
        Ok(Some((
            Self {
                stop,
                ended,
                thread: Some(thread),
            },
            DocWriter(writer),
        )))
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
    /// The last read's value and version while it was readable; `None`
    /// refuses every write (D-5).
    doc: Option<(Value, u64)>,
    live: Vec<String>,
    names: BTreeMap<String, String>,
    /// The folders last asked about, and when.
    #[cfg_attr(not(all(target_os = "ios", not(test))), allow(dead_code))]
    asked: Option<(Vec<String>, Instant)>,
    period: Duration,
    stats: WriteStats,
    /// The diagnostics code of the last read's result, recorded on change.
    reported: Option<&'static str>,
    /// This connection's herdr-core boot: the core id and the reader.
    boot: String,
    /// Spaces seen with every terminal ended (`ended_spaces`).
    tracker: Tracker,
    /// The removals the last read made due.
    due: Vec<Intent>,
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
        self.doc = None;
        let read = match core.doc_get(herdr_doc::NAME) {
            Ok(doc) => {
                self.held = Some(doc.version);
                match herdr_doc::parse_view(&doc.value) {
                    Ok(view) => {
                        self.doc = Some((doc.value, doc.version));
                        Read::Readable(view)
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Herdr 2 document");
                        Read::Unreadable
                    }
                }
            }
            Err(error) => Read::Unavailable(error.to_string()),
        };
        let code = match &read {
            Read::Readable(_) => "herdr2_doc_readable",
            Read::Unreadable => "herdr2_doc_unreadable",
            Read::Unavailable(_) => "herdr2_doc_unavailable",
        };
        if self.reported != Some(code) {
            crate::diagnostics::record(code);
            self.reported = Some(code);
        }
        // The full list, after the document: every terminal it names was
        // made before it was read. A failed list keeps the last one: a pane
        // is not dropped for it, and no space is removed.
        let listing = core.list(true).ok().map(|list| {
            let listing = Listing::new(&list.terminals);
            self.live = list
                .terminals
                .into_iter()
                .filter(|terminal| terminal.state == TerminalState::Live)
                .map(|terminal| terminal.address)
                .collect();
            listing
        });
        self.due = match (&self.doc, &read, listing) {
            (Some((value, _)), Read::Readable(view), Some(mut listing)) => {
                let unlisted = ended_spaces::unlisted(value, view, &listing);
                confirm_unlisted(core, &mut listing, unlisted, stopped);
                let found = ended_spaces::ended(value, view, &listing);
                self.tracker.observe(&self.boot, Instant::now(), found)
            }
            _ => {
                self.tracker.forget();
                Vec::new()
            }
        };
        if let Read::Readable(view) = &read {
            self.name_spaces(view, stopped);
        }
        let snapshot = DocSnapshot {
            version: self.held.unwrap_or_default(),
            read,
            live: self.live.clone(),
            names: self.names.clone(),
            stats: self.stats.clone(),
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

impl Reader {
    /// Takes out what the last read made due (`ended_spaces`), one space
    /// at a time, each applied to the latest document and read back before
    /// the next; a space changed since it was judged stays. The user is
    /// told nothing: no change of theirs was refused.
    fn remove_ended(&mut self, stopped: &impl Fn() -> bool) {
        for intent in std::mem::take(&mut self.due) {
            if stopped() {
                break;
            }
            let outcome = match self.send(&intent, stopped) {
                Outcome::Saved => {
                    self.stats.ended += 1;
                    crate::diagnostics::record("herdr2_doc_space_ended");
                    self.read(stopped);
                    "saved".to_owned()
                }
                Outcome::Unchanged => "unchanged".to_owned(),
                Outcome::Lost => "conflict-lost".to_owned(),
                Outcome::Refused(gate) => format!("check-failed:{gate}"),
                Outcome::NotReadable => "not-readable".to_owned(),
                Outcome::Unknown => "unknown".to_owned(),
            };
            tracing::info!(%outcome, "Herdr 2 ended panes removal");
        }
        // Reads in between judged again; what is still due comes back at
        // the next read.
        self.due.clear();
    }

    /// One intent, start to end; nothing else is sent meanwhile (D-6).
    fn write(&mut self, intent: Intent, stopped: &impl Fn() -> bool) {
        let outcome = self.send(&intent, stopped);
        let (last, notice) = match &outcome {
            Outcome::Unchanged => ("unchanged".to_owned(), None),
            Outcome::Saved => ("saved".to_owned(), None),
            Outcome::Lost => ("conflict-lost".to_owned(), Some(CHANGED_ELSEWHERE)),
            Outcome::Refused(gate) => (format!("check-failed:{gate}"), Some(REFUSED)),
            Outcome::NotReadable => ("not-readable".to_owned(), Some(NOT_READABLE)),
            Outcome::Unknown => ("unknown".to_owned(), None),
        };
        if let Outcome::Unknown = outcome {
            unsent().insert(self.host.clone(), intent);
        }
        tracing::info!(last = %last, "Herdr 2 document write");
        self.stats.last = Some(last);
        self.stats.notice = notice.map(str::to_owned);
        self.stats.finished += 1;
        self.read(stopped);
    }

    fn send(&mut self, intent: &Intent, stopped: &impl Fn() -> bool) -> Outcome {
        let mut sent: Option<Value> = None;
        for _ in 0..MAX_SENDS {
            let Some((value, version)) = self.doc.clone() else {
                return Outcome::NotReadable;
            };
            let candidate = match herdr_doc::apply(&value, intent) {
                Ok(Applied::Unchanged) => return Outcome::Unchanged,
                Ok(Applied::Changed(candidate)) => candidate,
                Err(error) => {
                    tracing::warn!(%error, "Herdr 2 document intent");
                    return Outcome::NotReadable;
                }
            };
            if let Err(gate) = herdr_doc::check_write(&value, &candidate, intent) {
                return Outcome::Refused(gate);
            }
            // Never the same value again under a newer version (D-3).
            if sent.as_ref() == Some(&candidate) {
                return Outcome::Lost;
            }
            match self
                .backend
                .core()
                .doc_set(herdr_doc::NAME, &candidate, version)
            {
                Ok(_) => {
                    self.stats.writes += 1;
                    return Outcome::Saved;
                }
                Err(CoreError::Refused {
                    kind: Some(kind), ..
                }) if kind == "conflict" => {
                    self.stats.conflicts += 1;
                    sent = Some(candidate);
                    self.read(stopped);
                }
                Err(error) => {
                    tracing::warn!(%error, "Herdr 2 document write");
                    return Outcome::Unknown;
                }
            }
        }
        Outcome::Lost
    }
}

/// Asks herdr-core, on the connection the list came from, about each
/// terminal the full list left out, as the official app does before it
/// removes a pane (`check_closed`): only a `not_found` refusal ends one.
/// Any other answer keeps it; an error or a timeout keeps it and every
/// terminal not asked about yet, until a later read (issue 222).
fn confirm_unlisted(
    core: &sheprd_core_client::Core,
    listing: &mut Listing,
    unlisted: Vec<String>,
    stopped: &impl Fn() -> bool,
) {
    for address in unlisted {
        if stopped() {
            return;
        }
        match core.get(&address) {
            Err(CoreError::Refused {
                kind: Some(kind), ..
            }) if kind == "not_found" => listing.not_found(&address),
            Ok(_) => {}
            Err(error) => {
                tracing::info!(%error, "Herdr 2 terminal check; panes kept");
                return;
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

#[cfg(all(test, unix))]
#[path = "herdr_doc_ended_live_tests.rs"]
mod ended_live_tests;
#[cfg(test)]
#[path = "herdr_doc_ended_tests.rs"]
mod ended_tests;
#[cfg(all(test, unix))]
#[path = "herdr_doc_live_tests.rs"]
mod live_tests;
#[cfg(test)]
#[path = "herdr_doc_migrate_tests.rs"]
mod migrate_tests;
#[cfg(test)]
#[path = "herdr_doc_sync_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "herdr_doc_write_tests.rs"]
mod write_tests;
