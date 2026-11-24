//! Herdr 2 workspace. Each connection owns an immutable mailbox and bounded FIFO.
#[cfg(test)]
mod discovery_tests;
#[cfg(all(test, unix))]
mod empty_space_tests;
mod ended;
#[cfg(test)]
mod image_tests;
#[cfg(all(test, unix))]
mod live_core_tests;
mod paste;
mod paste_path;
#[cfg(test)]
pub(crate) mod paste_test_support;
#[cfg(test)]
pub(crate) mod publication_probe;
#[cfg(test)]
mod regression_tests;
#[cfg(all(test, unix))]
mod saved_space_tests;
mod screen;
#[cfg(all(test, unix))]
mod space_fleet;
pub(crate) mod status;
mod store;
#[cfg(test)]
mod tests;
pub(crate) mod vacate;
#[cfg(test)]
mod vacate_tests;
mod worker;

use crate::{live::Live, terminal_input::Destination};
use sheprd_backend::{ClientPaneInputEvent, Size};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, SyncSender},
};

pub(crate) enum Command {
    Input(Destination, ClientPaneInputEvent),
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SessionInfo {
    pub(crate) name: String,
    pub(crate) tabs: Vec<TabInfo>,
    pub(crate) confirmed: bool,
    pub(crate) core_id: Option<String>,
    pub(crate) statuses: Vec<crate::shell::herdr2_frame::CoreStatus>,
    pub(crate) created: u64,
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
}

impl SessionLists {
    fn latest(&self) -> Option<&[SessionInfo]> {
        self.published.back().map(|(_, rows)| rows.as_slice())
    }

    fn publish(&mut self, epoch: u64, rows: Vec<SessionInfo>) {
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
    /// Spaces stopped here, kept off the rows until a poll no longer lists them.
    removed: std::collections::BTreeSet<String>,
    /// Spaces the last successful poll listed without a server answering.
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
    #[cfg(test)]
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
            poll,
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
            status::PollPlan::default(),
            discovery,
        )
    }

    fn start_worker(
        target: SessionTarget,
        (generation, inbox): (u64, Arc<Mutex<Live>>),
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
        poll: status::PollPlan,
        discovery: Discovery,
    ) -> crate::Result<Self> {
        let host = target.host.clone();
        let name = target.name.clone();
        let sessions = Arc::new(Mutex::new(SessionLists {
            published: Default::default(),
            discovery,
            current: name.clone(),
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
                let result = (|| {
                    let backend = connect()?;
                    // Hello may finish after this connection was retired. Never spawn,
                    // attach or persist a terminal for that obsolete completion.
                    if stopped.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    // Its own thread, never the worker loop below. Dropped
                    // (stopped, joined within status::JOIN_BOUND) when the
                    // worker returns.
                    let period = poll.period;
                    let _statuses = poll.listing(&backend).and_then(|list| {
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
                        receive,
                        root,
                    )
                })();
                stopped.store(true, Ordering::Release);
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
            #[cfg(test)]
            finished,
        })
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
}

/// The session itself, then the other spaces its host last showed, in the
/// sidebar's order.
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

#[cfg(any(target_os = "ios", test))]
fn record_discovery(
    lists: &SessionList,
    root: Option<&std::path::Path>,
    host: &str,
    output: Option<&[u8]>,
) -> crate::Result<bool> {
    record_discovery_at(lists, root, host, output, std::time::Instant::now())
}

/// `record_discovery` for the snapshot taken at `now`. The spaces it found
/// vacant are noted in `Discovery::vacancies`; `due_vacancies` names those
/// that stayed so long enough to be stopped.
#[cfg(any(target_os = "ios", test))]
fn record_discovery_at(
    lists: &SessionList,
    root: Option<&std::path::Path>,
    host: &str,
    output: Option<&[u8]>,
    now: std::time::Instant,
) -> crate::Result<bool> {
    let observed = output.map(parse_discovery);
    // The names-only listing (no python3 on the host) cannot say whether a
    // listed space's server answers.
    let answers_seen = output.is_some_and(|output| !output.starts_with(b"\0"));
    // Spaces holding a terminal whose state is not finished: never vacant,
    // whatever the saved tabs say (a state this client does not know counts).
    let busy = observed
        .iter()
        .flatten()
        .filter(|(_, row)| {
            row.as_ref()
                .is_some_and(|row| !vacate::vacant(0, &row.terminals))
        })
        .map(|(name, _)| name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let listed = observed
        .iter()
        .flat_map(|observed| observed.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    let rows = observed.map(|observed| worker::catalog_sessions(root, host, observed));
    let mut guard = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current = guard.current.clone();
    let discovery = &mut guard.discovery;
    let mut gone = Vec::new();
    let changed = 'snapshot: {
        let Some(rows) = rows else {
            break 'snapshot None;
        };
        discovery.failed = false;
        let mut rows = rows?;
        // A space stopped here may still be listed by a poll that began before.
        discovery.removed.retain(|name| listed.contains(name));
        rows.retain(|row| !discovery.removed.contains(&row.name));
        // A saved layout whose server this successful poll does not find is
        // a space that is gone (stopped elsewhere, or by `hc server stop`):
        // it leaves the sidebar and its layout file is deleted. A space the
        // poll lists but whose server did not answer must stay so in two polls
        // in a row, so a busy server's slow answer never costs a layout.
        let mut unanswered = std::collections::BTreeSet::new();
        for row in rows
            .iter()
            .filter(|row| !row.confirmed && row.name != current && !vacate::protected(&row.name))
        {
            if !listed.contains(&row.name) {
                gone.push(row.name.clone());
            } else if answers_seen {
                if discovery.unanswered.contains(&row.name) {
                    gone.push(row.name.clone());
                }
                unanswered.insert(row.name.clone());
            }
        }
        discovery.unanswered = unanswered;
        rows.retain(|row| !gone.contains(&row.name));
        let vacant = rows
            .iter()
            .filter(|row| {
                row.confirmed
                    && !vacate::protected(&row.name)
                    && !busy.contains(&row.name)
                    && row.tabs.is_empty()
            })
            .filter_map(|row| Some((row.name.clone(), row.core_id.clone()?)))
            .collect();
        discovery.vacancies.observe(now, vacant);
        if discovery.rows.as_ref() == Some(&rows) {
            Some(false)
        } else {
            discovery.rows = Some(rows);
            Some(true)
        }
    };
    drop(guard);
    if let Some(changed) = changed {
        if !gone.is_empty() {
            let _storage = store::LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for name in &gone {
                if let Err(error) = store::remove_workspace(root, host, name) {
                    tracing::warn!(%error, %name, "Forget a Herdr 2 space that is gone");
                }
            }
        }
        return Ok(changed || !gone.is_empty());
    }

    // A failed poll (transport error or timeout) deletes nothing and forgets
    // nothing: rows and layouts stay.
    let mut lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let discovery = &mut lists.discovery;
    discovery.unanswered.clear();
    // A single transport failure leaves the last snapshot usable.
    if !std::mem::replace(&mut discovery.failed, true) {
        return Ok(false);
    }
    let Some(rows) = &mut discovery.rows else {
        return Ok(false);
    };
    let mut changed = false;
    for row in rows.iter_mut().filter(|row| row.confirmed) {
        row.mark_unconfirmed();
        changed = true;
    }
    Ok(changed)
}

/// The spaces (name, server id) the polls found vacant at two snapshots at
/// least `vacate::GRACE` apart, other than `current`, which its own worker
/// removes.
#[cfg(any(target_os = "ios", test))]
fn due_vacancies(
    lists: &SessionList,
    now: std::time::Instant,
    current: &str,
) -> Vec<(String, String)> {
    let lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    lists
        .discovery
        .vacancies
        .due(now)
        .into_iter()
        .filter(|(name, _)| name != current)
        .collect()
}

/// Stops one vacant space found by a poll and forgets it: its saved layout,
/// its row, its timer. On a failure the space stays listed and the error is
/// the result. Blocking: a worker thread only.
#[cfg(any(target_os = "ios", test))]
fn retire_vacant(
    lists: &SessionList,
    root: Option<&std::path::Path>,
    host: &str,
    (name, core_id): &(String, String),
    stop: &vacate::Stopper,
) -> crate::Result<()> {
    if let Err(error) = stop(name, core_id) {
        let mut lists = lists
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        lists.discovery.vacancies.failed(name, core_id);
        return Err(error);
    }
    {
        let _storage = store::LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(error) = store::remove_workspace(root, host, name) {
            tracing::warn!(%error, %name, "Forget a removed Herdr 2 space");
        }
    }
    let mut lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let discovery = &mut lists.discovery;
    discovery.vacancies.done(name);
    discovery.removed.insert(name.clone());
    if let Some(rows) = &mut discovery.rows {
        rows.retain(|row| &row.name != name);
    }
    Ok(())
}

#[cfg(any(target_os = "ios", test))]
fn parse_discovery(output: &[u8]) -> Observations {
    let valid = |name: &str| {
        !name.is_empty()
            && name.trim() == name
            && !name.chars().any(char::is_control)
            && sheprd_client::core_connect_command(name).is_ok()
    };
    if let Some(names) = output.strip_prefix(b"\0") {
        return names
            .split(|byte| *byte == 0)
            .filter_map(|field| std::str::from_utf8(field).ok())
            .filter(|name| valid(name))
            .map(|name| (name.to_owned(), None))
            .collect();
    }
    output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            type WireRow = (
                String,
                Option<(String, Vec<sheprd_core_client::wire::Terminal>)>,
            );
            let (name, observation) = serde_json::from_slice::<WireRow>(line).ok()?;
            let observation = observation.and_then(|(core_id, terminals)| {
                (!core_id.is_empty()).then_some(ExternalObservation { core_id, terminals })
            });
            valid(&name).then_some((name, observation))
        })
        .collect()
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
                // server is stopped and its row leaves the sidebar.
                for vacant in due_vacancies(&mailbox, std::time::Instant::now(), &current) {
                    if stopped() {
                        break;
                    }
                    let result = retire_vacant(&mailbox, root.as_deref(), &host, &vacant, &stopper);
                    if let Ok(mut live) = inbox.lock() {
                        if let Err(error) = result {
                            live.status =
                                format!("Could not remove Herdr 2 space {}: {error}", vacant.0);
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
