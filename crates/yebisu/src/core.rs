//! Herdr 2 workspace. Each connection owns an immutable mailbox and bounded FIFO.
#[cfg(test)]
mod image_tests;
mod paste;
mod paste_path;
#[cfg(test)]
pub(crate) mod paste_test_support;
#[cfg(test)]
pub(crate) mod publication_probe;
#[cfg(test)]
mod regression_tests;
mod screen;
pub(crate) mod status;
mod store;
#[cfg(test)]
mod tests;
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

pub(crate) struct SessionTarget {
    pub(crate) host: String,
    pub(crate) name: String,
    pub(crate) focus: Option<String>,
    pub(crate) expected_core_id: Option<String>,
}

type SessionList = Arc<Mutex<SessionLists>>;

#[derive(Default)]
struct SessionLists {
    current: Option<(u64, Vec<SessionInfo>)>,
    #[cfg(any(target_os = "ios", test))]
    external: Option<Vec<SessionInfo>>,
}

pub(crate) struct Session {
    queue: SyncSender<Queued>,
    input_epoch: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    /// Herdr 2's terminal statuses for the sidebar (`status::StatusPoll`).
    statuses: status::StatusMailbox,
    name: String,
    sessions: SessionList,
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
            generation,
            inbox,
            connect,
            root,
            column,
            poll,
        )
    }

    pub(crate) fn start_named(
        target: SessionTarget,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
    ) -> crate::Result<Self> {
        Self::start_worker(
            target,
            generation,
            inbox,
            connect,
            root,
            column,
            status::PollPlan::default(),
        )
    }

    fn start_worker(
        target: SessionTarget,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
        column: crate::shell::single_pane::Mailbox,
        poll: status::PollPlan,
    ) -> crate::Result<Self> {
        let name = target.name.clone();
        let sessions = SessionList::default();
        let worker_sessions = sessions.clone();
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
                            target.host.clone(),
                            target.name.clone(),
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
                        (&shared, &worker_sessions),
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
            name,
            sessions,
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

    /// Match the catalog to the input epoch of the Live view the caller drew.
    pub(crate) fn sessions(&self, viewed_epoch: u64) -> Vec<SessionInfo> {
        let lists = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some((_, rows)) = lists
            .current
            .as_ref()
            .filter(|(epoch, _)| *epoch == viewed_epoch)
        else {
            return Vec::new();
        };
        #[cfg(any(target_os = "ios", test))]
        if let Some(external) = &lists.external {
            let mut merged = rows
                .iter()
                .filter(|row| row.name == self.name)
                .chain(external)
                .cloned()
                .collect::<Vec<_>>();
            sort_sessions(&mut merged);
            return merged;
        }
        rows.clone()
    }

    #[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
    pub(crate) fn publish_discovery(
        &self,
        root: Option<&std::path::Path>,
        host: &str,
        output: &[u8],
    ) -> crate::Result<()> {
        publish_discovery(
            &self.sessions,
            worker::inactive_sessions(root, host, self.name(), parse_discovery(output))?,
        );
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
fn publish_discovery(lists: &SessionList, rows: Vec<SessionInfo>) -> bool {
    let mut lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if lists.external.as_ref() == Some(&rows) {
        return false;
    }
    lists.external = Some(rows);
    true
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

#[cfg(all(target_os = "ios", not(test)))]
fn start_discovery(
    host: String,
    current: String,
    root: Option<std::path::PathBuf>,
    mailbox: SessionList,
    inbox: Arc<Mutex<Live>>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<()> {
    use std::{thread, time::Duration};

    // Query the existing daemon socket. hc connect and hc ls may start a core,
    // which is only appropriate for an explicit user connection.
    const DISCOVER: &str = concat!(
        r#"if command -v python3 >/dev/null 2>&1; then
    exec python3 - <<'YEBISU_CORE_DISCOVERY'
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

    thread::Builder::new()
        .name("yebisu-core-discovery".into())
        .spawn(move || {
            let stopped = || stop.load(Ordering::Acquire);
            while !stopped() {
                let observed = sheprd_client::run_ssh_command(
                    &host,
                    DISCOVER,
                    Duration::from_secs(3),
                    48_000,
                    stopped,
                )
                .map(|(exit, output)| {
                    if exit.success() {
                        parse_discovery(&output)
                    } else {
                        Observations::new()
                    }
                })
                .unwrap_or_default();
                if !stopped()
                    && let Ok(rows) =
                        worker::inactive_sessions(root.as_deref(), &host, &current, observed)
                    && publish_discovery(&mailbox, rows)
                    && let Ok(mut live) = inbox.lock()
                {
                    live.dirty = true;
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
