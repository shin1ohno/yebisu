//! Herdr 2 workspace. Each connection owns an immutable mailbox and bounded FIFO.
#[cfg(test)]
mod discovery_tests;
mod doc_tabs;
#[cfg(test)]
mod doc_tabs_tests;
#[cfg(all(test, unix))]
mod empty_space_tests;
mod ended;
mod ended_spaces;
#[cfg(test)]
mod ended_spaces_tests;
// The document write path (#144 D4) uses `apply`, `check_write` and `new_id`.
#[cfg_attr(not(test), allow(dead_code))]
mod herdr_doc;
#[cfg(test)]
mod herdr_doc_close_tests;
mod herdr_doc_prune;
#[cfg(test)]
mod herdr_doc_prune_tests;
#[cfg(test)]
mod herdr_doc_remove_tests;
pub(crate) mod herdr_doc_sync;
#[cfg(test)]
mod herdr_doc_tests;
#[cfg(test)]
mod image_tests;
#[cfg(all(test, unix))]
mod live_core_tests;
mod named_close;
#[cfg(all(test, unix))]
mod named_close_live_tests;
mod paste;
mod paste_path;
#[cfg(test)]
pub(crate) mod paste_test_support;
#[cfg(any(target_os = "ios", test))]
mod poll;
#[cfg(test)]
pub(crate) mod publication_probe;
#[cfg(test)]
mod regression_tests;
mod running;
#[cfg(all(test, unix))]
mod saved_space_tests;
mod screen;
#[cfg(all(test, unix))]
mod space_fleet;
pub(crate) mod status;
#[cfg(all(test, unix))]
mod stop_guard_tests;
mod store;
#[cfg(test)]
mod tests;
pub(crate) mod vacate;
#[cfg(test)]
mod vacate_tests;
mod worker;

use crate::{live::Live, terminal_input::Destination};
pub(crate) use doc_tabs::NO_CLOSE_DOCUMENT;
pub(crate) use named_close::PROTECTED as NAMED_CLOSE_PROTECTED;
use sheprd_backend::{ClientPaneInputEvent, Size};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, SyncSender},
};

pub(crate) enum Command {
    Input(Destination, ClientPaneInputEvent),
    /// Wheel input for `pane`, which need not be the focused pane; the
    /// focused `destination` proves the layout it was aimed at is current.
    Wheel {
        destination: Destination,
        pane: String,
        event: ClientPaneInputEvent,
    },
    PutImage {
        destination: Destination,
        ticket: crate::handoff_ticket::PasteTicket,
        bytes: Vec<u8>,
        reply: SyncSender<PasteOutcome>,
    },
    Resize(Size),
    Split,
    SplitInDirection(bool),
    NewTab,
    SelectTab(String),
    CloseTab(String),
    FocusAddress(String),
    /// Draws a document space of the `default` session, or the iPad-only
    /// tabs for `None` (#144 D2, owner answer D2-1).
    SelectDocSpace(Option<String>),
    /// Herdr 2's New space: a space in the `default` core's document, never
    /// another session (#144 decision 10).
    NewDocSpace,
    /// Renames a space of the `default` core's document; "" is the
    /// automatic name (#144 D3, Design E).
    RenameSpace {
        id: String,
        name: String,
    },
    /// Close Space (#231): space `id` leaves the `default` core's document
    /// and its terminals end.
    CloseDocSpace(String),
    /// Close Space on the connected `space-<ms>` session (#249): its live
    /// terminals end, then its server is stopped as an empty space is
    /// (`vacate.rs`). `asked` are the busy terminals the user agreed to end;
    /// another busy one refuses the close (`named_close.rs`).
    CloseNamedSpace {
        asked: Vec<String>,
    },
    /// Renames a tab of the `default` core's document; "" is its position.
    RenameTab {
        id: String,
        name: String,
    },
    Focus(usize),
    Orientation,
    #[cfg(test)]
    PauseInput(crate::handoff::PublicationGate),
    #[cfg(test)]
    ArmPublicationProbe(Box<publication_probe::AfterCompare>),
    #[cfg(test)]
    FinishPublicationProbe(publication_probe::Key, SyncSender<publication_probe::Key>),
}

pub(super) struct Queued {
    command: Command,
    input_epoch: u64,
}

pub(crate) struct PasteOutcome {
    pub serial: u64,
    pub result: Result<crate::handoff_ticket::Queued, String>,
}

/// The terminal panes and title that C2's tab row can draw for one tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabInfo {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) panes: Vec<String>,
    pub(crate) active: bool,
    pub(crate) focused_pane: usize,
    pub(crate) vertical: bool,
    /// A document tab without a name: `label` is its position (#144).
    pub(crate) auto_name: bool,
}

/// What the `herdr-app` document adds to the `default` session's row
/// (#144 D2). `None` on `SessionInfo` until the first read answered, and
/// for every other session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocCatalog {
    pub(crate) state: DocRows,
    /// The document space drawn; `None` while the iPad-only tabs are.
    pub(crate) selected: Option<String>,
    /// The iPad-only tabs: terminals of this core in no document block.
    pub(crate) local_tabs: usize,
    /// The version of the document read: a rename's name counts as shown
    /// once a later version is read (#144 D3, the optimistic label).
    pub(crate) version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DocRows {
    /// The document's spaces in its order.
    Readable(Vec<DocSpace>),
    Unreadable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocSpace {
    pub(crate) id: String,
    pub(crate) name: String,
    /// No name of its own: `name` is the folder's checkout (decision 9).
    pub(crate) auto_name: bool,
    pub(crate) folder: String,
    /// Every tab of the space; `panes` are all its terminals in order.
    pub(crate) tabs: Vec<TabInfo>,
    /// Its live terminals that run a program or an agent: Close Space
    /// asks first when there is one (#231).
    pub(crate) busy: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SessionInfo {
    pub(crate) name: String,
    pub(crate) tabs: Vec<TabInfo>,
    pub(crate) confirmed: bool,
    pub(crate) core_id: Option<String>,
    pub(crate) statuses: Vec<crate::shell::herdr2_frame::CoreStatus>,
    pub(crate) created: u64,
    pub(crate) doc: Option<DocCatalog>,
}

impl SessionInfo {
    fn mark_unconfirmed(&mut self) {
        self.confirmed = false;
        self.core_id = None;
        self.statuses.clear();
    }
}

pub(crate) struct SessionTarget {
    pub(crate) host: String,
    pub(crate) name: String,
    pub(crate) focus: Option<String>,
    pub(crate) expected_core_id: Option<String>,
}

type SessionList = Arc<Mutex<SessionLists>>;

/// Publications a view may still be drawing. The worker publishes the catalog
/// with the Live of the same epoch, but the view keeps drawing the Live it
/// took last until its next poll (every 33 ms, app.rs), so it can be several
/// publications behind. Each publication is one epoch change (a command or an
/// ended pane), so 64 covers far more than any view lags between two polls.
const PUBLICATION_BOUND: usize = 64;

#[derive(Default)]
struct SessionLists {
    /// Oldest first; epochs only increase within one session.
    published: std::collections::VecDeque<(u64, Vec<SessionInfo>)>,
    discovery: Discovery,
    /// The session the worker serves: its layout and row are the worker's own.
    current: String,
    /// A failed stop, shown until the space is removed or the user acts: the
    /// worker puts it over every screen it publishes (`worker.rs`).
    notice: Option<String>,
}

impl SessionLists {
    fn latest(&self) -> Option<&[SessionInfo]> {
        self.published.back().map(|(_, rows)| rows.as_slice())
    }

    /// Rows for a new epoch, or new rows for the epoch already published:
    /// a document read that changes no drawn pane keeps its epoch (#144), so
    /// input aimed at that surface is not dropped as stale.
    fn publish(&mut self, epoch: u64, rows: Vec<SessionInfo>) {
        if let Some((latest, current)) = self.published.back_mut()
            && *latest == epoch
        {
            *current = rows;
            return;
        }
        if self.published.len() == PUBLICATION_BOUND {
            self.published.pop_front();
        }
        self.published.push_back((epoch, rows));
    }

    /// The rows published with `viewed_epoch`. A view only takes a newer Live
    /// after drawing this one, so older publications are dropped.
    fn viewed(&mut self, viewed_epoch: u64) -> Option<&[SessionInfo]> {
        let index = self
            .published
            .iter()
            .position(|(epoch, _)| *epoch == viewed_epoch)?;
        self.published.drain(..index);
        self.published.front().map(|(_, rows)| rows.as_slice())
    }
}

#[derive(Clone, Default)]
pub(crate) struct Discovery {
    rows: Option<Vec<SessionInfo>>,
    failed: bool,
    /// Spaces the poll found vacant, to stop once they stay so (`vacate.rs`).
    vacancies: vacate::Vacancies,
    /// Spaces stopped here, with the pid of the server stopped: kept off the
    /// rows while their server does not answer (or is that same server), and
    /// back once another client has started it again and it runs a terminal.
    removed: std::collections::BTreeMap<String, Option<u64>>,
    /// Spaces the last successful poll found absent (no socket, nothing
    /// listening) though their state remains: two in a row delete the layout.
    unanswered: std::collections::BTreeSet<String>,
}

pub(crate) struct Session {
    queue: SyncSender<Queued>,
    input_epoch: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    /// Herdr 2's terminal statuses for the sidebar (`status::StatusPoll`).
    statuses: status::StatusMailbox,
    host: String,
    name: String,
    sessions: SessionList,
    /// Set once by the worker when it removed its own space.
    removal: Arc<Mutex<Option<vacate::Removal>>>,
    /// Set by the worker thread when it returns, after any failure reason
    /// is in `live`.
    pub finished: Arc<AtomicBool>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl Session {
    #[cfg(test)]
    pub fn start(
        host: String,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
    ) -> crate::Result<Self> {
        Self::start_with(
            host,
            generation,
            inbox,
            connect,
            root,
            column,
            status::PollPlan::default(),
        )
    }

    /// `start` with the status poll `poll`: the application lists through
    /// the session's connection; tests choose their own listing.
    #[cfg(test)]
    pub(crate) fn start_with(
        host: String,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
        poll: status::PollPlan,
    ) -> crate::Result<Self> {
        Self::start_worker(
            SessionTarget {
                host,
                name: "default".into(),
                focus: None,
                expected_core_id: None,
            },
            (generation, inbox),
            connect,
            root,
            column,
            (poll, herdr_doc_sync::DocPlan::default()),
            Discovery::default(),
        )
    }

    /// `start` for the `default` session with the document reads `doc`.
    #[cfg(test)]
    pub(crate) fn start_doc(
        host: String,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
        doc: herdr_doc_sync::DocPlan,
    ) -> crate::Result<Self> {
        Self::start_worker(
            SessionTarget {
                host,
                name: "default".into(),
                focus: None,
                expected_core_id: None,
            },
            (generation, inbox),
            connect,
            root,
            column,
            (status::PollPlan::default(), doc),
            Discovery::default(),
        )
    }

    pub(crate) fn start_named(
        target: SessionTarget,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
        discovery: Discovery,
    ) -> crate::Result<Self> {
        Self::start_worker(
            target,
            (generation, inbox),
            connect,
            root,
            column,
            (
                status::PollPlan::default(),
                herdr_doc_sync::DocPlan::default(),
            ),
            discovery,
        )
    }

    fn start_worker(
        target: SessionTarget,
        (generation, inbox): (u64, Arc<Mutex<Live>>),
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
        (poll, doc): (status::PollPlan, herdr_doc_sync::DocPlan),
        discovery: Discovery,
    ) -> crate::Result<Self> {
        let host = target.host.clone();
        let name = target.name.clone();
        let sessions = Arc::new(Mutex::new(SessionLists {
            published: Default::default(),
            discovery,
            current: name.clone(),
            notice: None,
        }));
        let worker_sessions = sessions.clone();
        let removal = Arc::new(Mutex::new(None));
        let worker_removal = removal.clone();
        let (queue, receive) = mpsc::sync_channel(256);
        let stop = Arc::new(AtomicBool::new(false));
        let input_epoch = Arc::new(AtomicU64::new(0));
        let worker_epoch = input_epoch.clone();
        let shared = inbox.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let completed = finished.clone();
        let stopped = stop.clone();
        let statuses = status::StatusMailbox::default();
        let mailbox = statuses.clone();
        std::thread::Builder::new()
            .name("yebisu-core".into())
            .spawn(move || {
                let mut dialed = false;
                let result = (|| {
                    crate::diagnostics::record("herdr2_connecting");
                    let backend = connect()?;
                    dialed = true;
                    // Hello may finish after this connection was retired. Never spawn,
                    // attach or persist a terminal for that obsolete completion.
                    if stopped.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    crate::diagnostics::record("herdr2_connected");
                    // Its own thread, never the worker loop below. Dropped
                    // (stopped, joined within status::JOIN_BOUND) when the
                    // worker returns.
                    let period = poll.period;
                    let _statuses = poll.listing(&backend, &mailbox).and_then(|list| {
                        status::StatusPoll::start(
                            list,
                            mailbox,
                            shared.clone(),
                            stopped.clone(),
                            period,
                        )
                        // The sidebar loses its statuses, not the session.
                        .inspect_err(|error| tracing::warn!(%error, "Herdr 2 status poll"))
                        .ok()
                    });
                    // The official app's document is the `default` core's
                    // (#144 Design A); its reader is dropped with the worker.
                    let doc_mailbox =
                        (target.name == "default").then(herdr_doc_sync::DocMailbox::default);
                    let reader = match &doc_mailbox {
                        Some(mailbox) => herdr_doc_sync::DocSync::start(
                            doc,
                            (&backend, &target.host),
                            mailbox.clone(),
                            shared.clone(),
                            stopped.clone(),
                        )
                        .inspect_err(|error| tracing::warn!(%error, "Herdr 2 document reader"))
                        .ok()
                        .flatten(),
                        None => None,
                    };
                    // Without a reader nothing is ever published to wait for.
                    let (_doc, doc) = match reader {
                        Some((reader, writer)) => (Some(reader), doc_mailbox.zip(Some(writer))),
                        None => (None, None),
                    };
                    #[cfg(all(target_os = "ios", not(test)))]
                    let _discovery = if cfg!(target_abi = "sim")
                        && std::env::var_os("YEBISU_CORE_QA").is_some()
                    {
                        None
                    } else {
                        start_discovery(
                            (target.host.clone(), target.name.clone()),
                            root.clone(),
                            worker_sessions.clone(),
                            shared.clone(),
                            stopped.clone(),
                        )
                        .inspect_err(|error| tracing::warn!(%error, "Herdr 2 discovery poll"))
                        .ok()
                    };
                    worker::run(
                        backend,
                        target,
                        generation,
                        (&shared, &worker_sessions, &worker_removal),
                        (&stopped, &worker_epoch, &column),
                        (receive, doc),
                        root,
                    )
                })();
                stopped.store(true, Ordering::Release);
                if let Err(error) = &result {
                    crate::diagnostics::record(ended_code(dialed, error));
                }
                if let Err(error) = result {
                    tracing::warn!(%error, "Herdr 2 connection stopped");
                    if let Ok(mut live) = shared.lock() {
                        live.connected = false;
                        live.status = error.to_string();
                        live.dirty = true;
                    }
                }
                completed.store(true, Ordering::Release);
            })?;
        Ok(Self {
            queue,
            input_epoch,
            stop,
            statuses,
            host,
            name,
            sessions,
            removal,
            finished,
        })
    }
    /// The worker has stopped: dial failure, a lost session or a retirement.
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
    // Cancels queued consumer commands. Already-published SDK transport writes
    // require an SDK writer fence and are not claimed cancelled by this epoch.
    pub fn invalidate_input(&self) {
        self.input_epoch.fetch_add(1, Ordering::AcqRel);
    }
    #[cfg(test)]
    pub(crate) fn probe_input_epoch(&self) -> u64 {
        self.input_epoch.load(Ordering::Acquire)
    }
    /// The statuses the poll listed last; `None` before the first list.
    pub(crate) fn statuses(&self) -> Option<Vec<crate::shell::herdr2_frame::CoreStatus>> {
        self.statuses.latest()
    }
    /// The live terminals the poll found running a program or an agent;
    /// `None` before the first list (Close Space asks from it, #249).
    pub(crate) fn busy(&self) -> Option<Vec<String>> {
        self.statuses.busy()
    }
    #[cfg(test)]
    pub(crate) fn statuses_mailbox(&self) -> status::StatusMailbox {
        self.statuses.clone()
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// The removal of this session's own space, once the worker did it: its
    /// last tab closed, no terminal ran in it, and its server was stopped.
    pub(crate) fn removal(&self) -> Option<vacate::Removal> {
        self.removal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Keep a snapshot for the next named session without sharing its mailbox.
    pub(crate) fn retire_for_switch(self, target: &SessionTarget) -> Discovery {
        self.stop.store(true, Ordering::Release);
        if self.host != target.host || self.name == target.name {
            return Discovery::default();
        }
        let lists = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut discovery = lists.discovery.clone();
        // The old current session may have been saved after the last poll.
        if let Some(rows) = &mut discovery.rows
            && !rows.iter().any(|row| row.name == self.name)
            && let Some(current) = lists.latest()
            && let Some(row) = current.iter().find(|row| row.name == self.name)
        {
            let mut row = row.clone();
            row.mark_unconfirmed();
            rows.push(row);
        }
        discovery
    }

    /// Match the catalog to the input epoch of the Live view the caller drew,
    /// even after the worker has published newer ones: never a newer
    /// catalog's rows with an older surface.
    pub(crate) fn sessions(&self, viewed_epoch: u64) -> Vec<SessionInfo> {
        let mut lists = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(rows) = lists.viewed(viewed_epoch).map(<[SessionInfo]>::to_vec) else {
            return Vec::new();
        };
        if lists.discovery.rows.is_some() {
            return merged_sessions(&lists, &self.name, &rows);
        }
        rows
    }

    #[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
    pub(crate) fn publish_discovery(
        &self,
        root: Option<&std::path::Path>,
        host: &str,
        output: &[u8],
    ) -> crate::Result<()> {
        record_discovery(&self.sessions, root, host, Some(output))?;
        Ok(())
    }

    pub fn send(&self, command: Command) -> crate::Result<()> {
        self.queue
            .try_send(Queued {
                command,
                input_epoch: self.input_epoch.load(Ordering::Acquire),
            })
            .map_err(|_| crate::Error::CoreQueue)
    }
}

#[cfg(all(target_os = "ios", not(test)))]
pub(crate) struct HostDiscovery {
    pub(crate) installed: bool,
}

/// Check whether a Herdr 2 binary is available on a chooser destination.
#[cfg(all(target_os = "ios", not(test)))]
pub(crate) fn discover_host(
    host: &str,
    cancelled: impl Fn() -> bool,
) -> crate::Result<HostDiscovery> {
    use std::time::Duration;
    const COMMAND: &str = r#"
for bin in "$(command -v hc 2>/dev/null || true)" "$(command -v herdr-core 2>/dev/null || true)" "$HOME/.local/bin/hc" "$HOME/.local/bin/herdr-core" "$HOME/.cargo/bin/hc" "$HOME/.cargo/bin/herdr-core" /opt/homebrew/bin/hc /opt/homebrew/bin/herdr-core /usr/local/bin/hc /usr/local/bin/herdr-core; do
    [ -n "$bin" ] && [ -f "$bin" ] && [ -x "$bin" ] || continue
    printf 'installed\000'
    exit 0
done
"#;
    let (exit, output) =
        sheprd_client::run_ssh_command(host, COMMAND, Duration::from_secs(3), 16_384, cancelled)
            .map_err(|error| crate::Error::Ssh(error.to_string()))?;
    if !exit.success() {
        return Err(crate::Error::Ssh("Herdr 2 discovery did not finish".into()));
    }
    Ok(HostDiscovery {
        installed: output == b"installed\0",
    })
}

/// The spaces `host` lists, by the discovery poll's own query (it never
/// starts a server). Used before reopening a saved space at launch.
#[cfg(target_os = "ios")]
pub(crate) fn probe_spaces(host: &str) -> crate::Result<std::collections::BTreeSet<String>> {
    let (exit, output) = sheprd_client::run_ssh_command(
        host,
        DISCOVER,
        std::time::Duration::from_secs(3),
        48_000,
        || false,
    )
    .map_err(|error| crate::Error::Ssh(error.to_string()))?;
    if !exit.success() {
        return Err(crate::Error::Ssh(
            "Herdr 2 space listing did not finish".into(),
        ));
    }
    Ok(poll::listed_spaces(&output))
}

#[cfg(test)]
pub(crate) fn listed_spaces(output: &[u8]) -> std::collections::BTreeSet<String> {
    poll::listed_spaces(output)
}

#[cfg(target_os = "ios")]
pub(crate) fn workspace_root() -> Option<std::path::PathBuf> {
    let root = sheprd_client::local_path::state_dir()?.join("herdr2");
    #[cfg(target_abi = "sim")]
    if std::env::var_os("YEBISU_CORE_QA").is_some() {
        // Keep a mock namespace across reconnects in the same process.
        return Some(root.join(format!("qa-{}", std::process::id())));
    }
    Some(root)
}

#[cfg(target_os = "ios")]
pub(crate) fn connect(host: &str, session: &str) -> crate::Result<sheprd_core_client::CoreBackend> {
    use std::time::Duration;
    let scan =
        sheprd_client::ssh::scan_host_key(host).map_err(|e| crate::Error::Ssh(e.to_string()))?;
    crate::hosts::authentication_gate(&scan)?;
    let command = sheprd_client::core_connect_command(session)
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    let (stream, tunnel) = sheprd_client::exec_ssh_stream(host, &command)
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    stream.set_read_timeout(Some(Duration::from_millis(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let stream = SshStream {
        stream,
        _tunnel: Arc::new(tunnel),
    };
    Ok(sheprd_core_client::CoreBackend::new(
        sheprd_core_client::Core::over(stream)?,
        10000,
    ))
}
#[cfg(target_os = "ios")]
struct SshStream {
    stream: sheprd_client::Stream,
    _tunnel: Arc<herdr_ssh::Tunnel>,
}
#[cfg(target_os = "ios")]
impl std::io::Read for SshStream {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.stream, bytes)
    }
}
#[cfg(target_os = "ios")]
impl std::io::Write for SshStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.stream, bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.stream)
    }
}

type Observations = std::collections::BTreeMap<String, Option<ExternalObservation>>;

struct ExternalObservation {
    core_id: String,
    terminals: Vec<sheprd_core_client::wire::Terminal>,
    /// The server's pid, to tell a restarted server from the one seen.
    pid: Option<u64>,
}

#[cfg(test)]
use poll::record_discovery_at;
#[cfg(any(target_os = "ios", test))]
use poll::{due_vacancies, record_discovery, retire_vacant};

/// The session itself, then the other spaces its host last showed, in the
/// sidebar's order.
/// The diagnostics code of a Herdr 2 link that ended with `error`.
fn ended_code(dialed: bool, error: &crate::Error) -> &'static str {
    match error {
        _ if !dialed => "herdr2_dial_failed",
        crate::Error::Core(sheprd_core_client::Error::Lost) => "herdr2_ended_lost",
        _ => "herdr2_ended_error",
    }
}

fn merged_sessions(lists: &SessionLists, own: &str, rows: &[SessionInfo]) -> Vec<SessionInfo> {
    let Some(external) = &lists.discovery.rows else {
        return rows.to_vec();
    };
    let mut merged = rows
        .iter()
        .filter(|row| row.name == own)
        .chain(external.iter().filter(|row| row.name != own))
        .cloned()
        .collect::<Vec<_>>();
    sort_sessions(&mut merged);
    merged
}

fn sort_sessions(rows: &mut [SessionInfo]) {
    rows.sort_by(|left, right| {
        (left.created == 0, left.created, &left.name).cmp(&(
            right.created == 0,
            right.created,
            &right.name,
        ))
    });
}

// Query existing daemon sockets. hc connect and hc ls may start a core,
// which is only appropriate for an explicit user connection.
const DISCOVER: &str = concat!(
    r#"if command -v python3 >/dev/null 2>&1; then
    exec python3 -I - <<'YEBISU_CORE_DISCOVERY'
"#,
    include_str!("shell/herdr2_frame/discover.py"),
    r#"
YEBISU_CORE_DISCOVERY
fi
printf '\000'
for session in "${XDG_STATE_HOME:-$HOME/.local/state}/herdr-core/"*; do
    [ -d "$session" ] && printf '%s\000' "${session##*/}"
done
exit 0
"#,
);

#[cfg(all(target_os = "ios", not(test)))]
fn start_discovery(
    (host, current): (String, String),
    root: Option<std::path::PathBuf>,
    mailbox: SessionList,
    inbox: Arc<Mutex<Live>>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<()> {
    use std::{thread, time::Duration};

    let stopper = vacate::stopper(&host);
    thread::Builder::new()
        .name("yebisu-core-discovery".into())
        .spawn(move || {
            let stopped = || stop.load(Ordering::Acquire);
            while !stopped() {
                let output = sheprd_client::run_ssh_command(
                    &host,
                    DISCOVER,
                    Duration::from_secs(3),
                    48_000,
                    stopped,
                )
                .ok()
                .and_then(|(exit, output)| exit.success().then_some(output));
                if !stopped()
                    && let Ok(true) =
                        record_discovery(&mailbox, root.as_deref(), &host, output.as_deref())
                    && let Ok(mut live) = inbox.lock()
                {
                    live.dirty = true;
                }
                // A space found vacant at two polls, 5 s apart, goes: its
                // server is stopped (if still empty) and its row leaves the
                // sidebar. A failure is the screen's notice until the space
                // goes or the user acts.
                for vacant in due_vacancies(&mailbox, std::time::Instant::now(), &current) {
                    if stopped() {
                        break;
                    }
                    let result = retire_vacant(&mailbox, root.as_deref(), &host, &vacant, &stopper);
                    if let Ok(mut live) = inbox.lock() {
                        if let Err(error) = &result {
                            tracing::warn!(%error, space = %vacant.0, "Herdr 2 space did not stop");
                            live.status = mailbox
                                .lock()
                                .ok()
                                .and_then(|lists| lists.notice.clone())
                                .unwrap_or_default();
                        }
                        live.dirty = true;
                    }
                }
                // The active session still uses its own one-second listing.
                for _ in 0..50 {
                    if stopped() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        })
        .map(drop)
}
