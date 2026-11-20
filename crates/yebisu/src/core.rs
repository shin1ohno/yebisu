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
}

pub(crate) struct SessionTarget {
    pub(crate) host: String,
    pub(crate) name: String,
    pub(crate) focus: Option<String>,
}

type SessionList = Arc<Mutex<Option<(u64, Vec<SessionInfo>)>>>;

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
                    let discovered = if cfg!(target_abi = "sim")
                        && std::env::var_os("YEBISU_CORE_QA").is_some()
                    {
                        Vec::new()
                    } else {
                        discover_host(&target.host, || stopped.load(Ordering::Acquire))
                            .inspect_err(|error| tracing::warn!(%error, "List Herdr 2 sessions"))
                            .map_or_else(|_| Vec::new(), |host| host.sessions)
                    };
                    #[cfg(any(not(target_os = "ios"), test))]
                    let discovered = Vec::new();
                    worker::run(
                        backend,
                        (target, discovered),
                        generation,
                        (&shared, &worker_sessions),
                        (&stopped, &worker_epoch, &column),
                        receive,
                        root,
                    )
                })();
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
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|(epoch, _)| *epoch == viewed_epoch)
            .map_or_else(Vec::new, |(_, sessions)| sessions.clone())
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
    pub(crate) sessions: Vec<String>,
}

/// herdr-core keeps its named namespaces under XDG_STATE_HOME/herdr-core.
/// Read those directories through the pinned SSH client without starting
/// any daemon. The SDK validates each name again when opening a session.
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
    for session in "${XDG_STATE_HOME:-$HOME/.local/state}/herdr-core/"*; do
        [ -d "$session" ] && printf '%s\000' "${session##*/}"
    done
    exit 0
done
"#;
    let (exit, output) =
        sheprd_client::run_ssh_command(host, COMMAND, Duration::from_secs(3), 16_384, cancelled)
            .map_err(|error| crate::Error::Ssh(error.to_string()))?;
    if !exit.success() {
        return Err(crate::Error::Ssh("Herdr 2 discovery did not finish".into()));
    }
    let mut fields = output.split(|byte| *byte == 0);
    let installed = fields.next() == Some(b"installed");
    let sessions = if installed {
        fields
            .filter_map(|field| {
                let name = std::str::from_utf8(field).ok()?;
                if name.is_empty()
                    || name.chars().any(char::is_control)
                    || sheprd_client::core_connect_command(name).is_err()
                {
                    return None;
                }
                Some(name.to_owned())
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(HostDiscovery {
        installed,
        sessions,
    })
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
