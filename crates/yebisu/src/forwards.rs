//! Port forwards: a saved host's loopback port carried to `127.0.0.1` on this
//! iPad over SSH, for Safari to open (docs/port-forward.md). Sheprd's
//! `port_forward.rs` is the model for the rules: a port of one host is
//! forwarded once, at most [`MAX_FORWARDS`] at a time, an ended forward stays
//! until dismissed or started again, and nothing reconnects on its own.
//!
//! The UI thread only starts, stops, pauses and resumes forwards and takes
//! their workers' latest reports on its 33 ms poll; every step that blocks
//! runs in [`worker`]. The list is not saved: each process starts with none
//! (owner decision 8). While the app is off screen a forward pauses after
//! [`PAUSE_GRACE`] and resumes on its previous local port when the app is
//! active again (owner decision 6).
mod error;
pub(crate) mod scan;
pub(crate) mod worker;

pub(crate) use error::{InputError, Reason};

use crate::app::Smoke;
use gpui::{AppLifecyclePhase, Context};
use std::{
    num::NonZeroU16,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use worker::{Command, Connector, Handle, Report, Start};

#[cfg(target_os = "ios")]
use herdr_uikit::BackgroundTask;
#[cfg(all(test, not(target_os = "ios")))]
use tests::BackgroundTask;

/// Forwards at once, across every host (Sheprd's limit).
pub(crate) const MAX_FORWARDS: usize = 32;
/// How long a forward keeps listening after the app leaves the screen.
pub(crate) const PAUSE_GRACE: Duration = Duration::from_secs(5);
/// How much longer than the grace the background task waits for every
/// forward to close.
const PAUSE_MARGIN: Duration = Duration::from_secs(2);
/// How often the background task's holder checks the forwards.
const PAUSER_NAP: Duration = Duration::from_millis(50);
/// Shown once per process, with the first Open.
pub(crate) const OPEN_HINT: &str = "Keep yebisu on screen beside Safari, or the forward pauses.";

/// Where a forward is, as its row shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Checking the host key, connecting, or probing the host's port.
    Connecting,
    Listening {
        local: u16,
        /// Nothing accepted on the host's port when the forward opened.
        nobody_yet: bool,
    },
    /// Closed while the app was off screen; resumes when it is active.
    Paused,
    /// Ended other than by the user; never restarts on its own.
    Ended(Reason),
}

impl State {
    /// The state's name in `qa.state` and logs.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Listening { .. } => "listening",
            Self::Paused => "paused",
            Self::Ended(_) => "ended",
        }
    }

    /// Whether a worker runs for it.
    fn live(self) -> bool {
        matches!(self, Self::Connecting | Self::Listening { .. })
    }
}

pub(crate) struct Forward {
    /// The saved host's SSH destination, its identity.
    pub(crate) host: String,
    pub(crate) remote: NonZeroU16,
    pub(crate) state: State,
    /// The local port it last listened on, tried first when it resumes.
    pub(crate) last_local: Option<u16>,
    worker: Option<Handle>,
}

impl Forward {
    /// The page Open shows: the address, not `localhost`, which Safari may
    /// try as `::1` first, where the forward does not listen.
    pub(crate) fn url(&self) -> Option<String> {
        match self.state {
            State::Listening { local, .. } => Some(format!("http://127.0.0.1:{local}/")),
            _ => None,
        }
    }

    /// What VoiceOver reads for the forward's row.
    pub(crate) fn label(&self) -> String {
        let remote = self.remote;
        match self.state {
            State::Connecting => format!("Port {remote} forward connecting"),
            State::Listening { local, .. } => {
                format!("Port {remote} forwarded to 127.0.0.1:{local}")
            }
            State::Paused => format!("Port {remote} forward paused"),
            State::Ended(_) => format!("Port {remote} forward ended"),
        }
    }
}

/// What a remote port typed in the forward sheet means (Sheprd's
/// `parse_port`): ASCII digits only, 1 to 65535, surrounding spaces ignored.
pub(crate) fn parse_port(text: &str) -> Result<NonZeroU16, InputError> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(InputError::Port);
    }
    text.parse().map_err(|_| InputError::Port)
}

/// What one poll took from the workers.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Polled {
    /// Status-line texts, in the order they happened.
    pub(crate) notices: Vec<String>,
    pub(crate) changed: bool,
}

/// Every forward of this process.
pub(crate) struct Forwards {
    entries: Vec<Forward>,
    connector: Arc<dyn Connector>,
    bind: worker::Bind,
    /// Set to release the background task early, when the app came back.
    pausing: Option<Arc<AtomicBool>>,
    /// The app is off screen: a forward that reports paused stays paused.
    background: bool,
    open_hint_shown: bool,
}

impl Forwards {
    /// The app's forwards, over its SSH connector.
    pub(crate) fn new() -> Self {
        #[cfg(target_os = "ios")]
        let connector: Arc<dyn Connector> = Arc::new(worker::SshConnector);
        #[cfg(not(target_os = "ios"))]
        let connector: Arc<dyn Connector> = Arc::new(worker::Unavailable);
        Self::with(connector, worker::listen)
    }

    /// Forwards over `connector`, listening with `bind`; for tests.
    pub(crate) fn with(connector: Arc<dyn Connector>, bind: worker::Bind) -> Self {
        Self {
            entries: Vec::new(),
            connector,
            bind,
            pausing: None,
            background: false,
            open_hint_shown: false,
        }
    }

    /// Every forward, in the order they were started.
    pub(crate) fn entries(&self) -> &[Forward] {
        &self.entries
    }

    /// `host`'s forwards, in the order they were started.
    pub(crate) fn for_host<'a>(&'a self, host: &'a str) -> impl Iterator<Item = &'a Forward> + 'a {
        self.entries
            .iter()
            .filter(move |forward| forward.host == host)
    }

    /// The forward of `host`'s `remote` port.
    pub(crate) fn get(&self, host: &str, remote: u16) -> Option<&Forward> {
        self.entries
            .iter()
            .find(|forward| forward.host == host && forward.remote.get() == remote)
    }

    /// Starts forwarding `host`'s `remote` port. A port already forwarded
    /// from the host is refused unless its forward ended, which is replaced;
    /// then at most [`MAX_FORWARDS`] may exist (Sheprd's `insert`).
    pub(crate) fn start(&mut self, host: &str, remote: NonZeroU16) -> Result<(), InputError> {
        let same = |forward: &Forward| forward.host == host && forward.remote == remote;
        if self
            .entries
            .iter()
            .any(|forward| same(forward) && !matches!(forward.state, State::Ended(_)))
        {
            return Err(InputError::Duplicate(remote.get()));
        }
        self.entries.retain(|forward| !same(forward));
        if self.entries.len() >= MAX_FORWARDS {
            return Err(InputError::Limit(MAX_FORWARDS));
        }
        let mut forward = Forward {
            host: host.to_owned(),
            remote,
            state: State::Connecting,
            last_local: None,
            worker: None,
        };
        self.run(&mut forward, sheprd_client::preferred_local_port(remote));
        self.entries.push(forward);
        Ok(())
    }

    /// Stops a forward; its row goes away. Dropping the worker's handle
    /// closes its listener and connection.
    pub(crate) fn stop(&mut self, host: &str, remote: u16) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|forward| forward.host != host || forward.remote.get() != remote);
        self.entries.len() != before
    }

    /// Removes an ended forward's row.
    pub(crate) fn dismiss(&mut self, host: &str, remote: u16) -> bool {
        self.stop(host, remote)
    }

    /// Stops the forwards of hosts no longer saved, or whose destination
    /// changed. Returns whether any stopped.
    pub(crate) fn retain_hosts(&mut self, hosts: &[String]) -> bool {
        let before = self.entries.len();
        self.entries.retain(|forward| hosts.contains(&forward.host));
        self.entries.len() != before
    }

    /// The page to open for a listening forward, with [`OPEN_HINT`] on the
    /// first Open of this process.
    pub(crate) fn open(
        &mut self,
        host: &str,
        remote: u16,
    ) -> Option<(String, Option<&'static str>)> {
        let url = self.get(host, remote)?.url()?;
        let hint = (!std::mem::replace(&mut self.open_hint_shown, true)).then_some(OPEN_HINT);
        Some((url, hint))
    }

    /// Whether a worker still runs.
    pub(crate) fn has_live(&self) -> bool {
        self.entries.iter().any(|forward| forward.state.live())
    }

    /// The app left the screen at `now`. Every running forward closes after
    /// [`PAUSE_GRACE`], or at once when `begin` gets no background time from
    /// the system (the app may be suspended any moment). The task `begin`
    /// returns is held until every forward has closed, or for the grace
    /// and a margin. A second call before [`Forwards::active`] changes
    /// nothing.
    pub(crate) fn background<T: Send + 'static>(
        &mut self,
        now: Instant,
        begin: impl FnOnce() -> Option<T>,
    ) {
        if std::mem::replace(&mut self.background, true) || !self.has_live() {
            return;
        }
        let task = begin();
        let deadline = if task.is_some() {
            now + PAUSE_GRACE
        } else {
            now
        };
        let mut done = Vec::new();
        for forward in self
            .entries
            .iter_mut()
            .filter(|forward| forward.state.live())
        {
            if let Some(worker) = forward.worker.as_mut() {
                worker.send(Command::PauseAt(deadline));
                done.push(worker.done());
            }
        }
        if let Some(task) = task {
            let cancel = Arc::new(AtomicBool::new(false));
            hold(task, done, deadline + PAUSE_MARGIN, Arc::clone(&cancel));
            self.pausing = Some(cancel);
        }
    }

    /// The app is active again: forwards still within their grace keep
    /// listening, and paused ones resume, each on its previous local port
    /// first.
    pub(crate) fn active(&mut self) {
        if !std::mem::replace(&mut self.background, false) {
            return;
        }
        if let Some(cancel) = self.pausing.take() {
            cancel.store(true, Ordering::Release);
        }
        let mut entries = std::mem::take(&mut self.entries);
        for forward in &mut entries {
            match forward.state {
                State::Paused => self.resume(forward),
                state if state.live() => {
                    if let Some(worker) = forward.worker.as_mut() {
                        worker.send(Command::CancelPause);
                    }
                }
                _ => {}
            }
        }
        self.entries = entries;
    }

    /// Takes each worker's latest report. `name` gives a host's display
    /// name for the status line.
    pub(crate) fn poll(&mut self, name: impl Fn(&str) -> String) -> Polled {
        let mut polled = Polled::default();
        let mut entries = std::mem::take(&mut self.entries);
        for forward in &mut entries {
            let Some(worker) = forward.worker.as_mut() else {
                continue;
            };
            worker.resend();
            let Some(report) = worker.take() else {
                continue;
            };
            polled.changed = true;
            let remote = forward.remote.get();
            match report {
                Report::Listening {
                    local,
                    nobody_yet,
                    wanted,
                } => {
                    forward.state = State::Listening { local, nobody_yet };
                    forward.last_local = Some(local);
                    polled.notices.push(listening_text(
                        remote,
                        local,
                        wanted,
                        nobody_yet.then(|| name(&forward.host)).as_deref(),
                    ));
                }
                Report::Paused { last_local } => {
                    forward.state = State::Paused;
                    forward.last_local = Some(last_local);
                    forward.worker = None;
                    // Paused just as the app came back: resume at once.
                    if !self.background {
                        self.resume(forward);
                    }
                }
                Report::Ended(reason) => {
                    forward.state = State::Ended(reason);
                    forward.worker = None;
                    let text = reason.text(&name(&forward.host), remote);
                    polled
                        .notices
                        .push(format!("Port {remote} forward ended: {text}"));
                }
            }
        }
        self.entries = entries;
        polled
    }

    fn resume(&self, forward: &mut Forward) {
        let preferred = forward
            .last_local
            .unwrap_or_else(|| sheprd_client::preferred_local_port(forward.remote));
        self.run(forward, preferred);
    }

    /// Starts `forward`'s worker, trying `preferred` first.
    fn run(&self, forward: &mut Forward, preferred: u16) {
        let start = Start {
            host: forward.host.clone(),
            remote: forward.remote,
            preferred,
            connector: Arc::clone(&self.connector),
            bind: self.bind,
        };
        match Handle::spawn(start) {
            Ok(worker) => {
                forward.worker = Some(worker);
                forward.state = State::Connecting;
            }
            Err(error) => {
                tracing::warn!(kind = ?error.kind(), "port forward worker did not start");
                forward.worker = None;
                forward.state = State::Ended(Reason::ConnectTimeout);
            }
        }
    }
}

/// The status line for a forward that opened. `host` names the host when
/// nothing accepted on its port yet.
fn listening_text(remote: u16, local: u16, wanted: u16, host: Option<&str>) -> String {
    let mut text = if local == wanted {
        format!("Forwarding port {remote} to 127.0.0.1:{local}")
    } else {
        format!("Port {remote} now forwards to 127.0.0.1:{local} ({wanted} is in use on this iPad)")
    };
    if let Some(host) = host {
        text.push_str(&format!(" · nothing listens on {remote} on {host} yet"));
    }
    text
}

/// Holds the background task on a thread of its own until every forward's
/// worker has returned, `until` passes, or the app came back. UIKit lets a
/// background task end on any thread.
fn hold<T: Send + 'static>(
    task: T,
    done: Vec<Arc<AtomicBool>>,
    until: Instant,
    cancel: Arc<AtomicBool>,
) {
    let spawned = thread::Builder::new()
        .name("yebisu-forward-pause".into())
        .spawn(move || {
            while !cancel.load(Ordering::Acquire)
                && Instant::now() < until
                && !done.iter().all(|done| done.load(Ordering::Acquire))
            {
                thread::sleep(PAUSER_NAP);
            }
            drop(task);
        });
    if let Err(error) = spawned {
        // The closure, and the task with it, was dropped: the system may
        // suspend the app before the forwards close.
        tracing::warn!(kind = ?error.kind(), "port forward pause holder did not start");
    }
}

impl Smoke {
    /// The 33 ms poll's share: forwards of hosts no longer saved stop, and
    /// each worker's news reaches the rows and the status line.
    pub(crate) fn poll_forwards(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.forwards.retain_hosts(&self.setup.hosts);
        let setup = &self.setup;
        let polled = self
            .forwards
            .poll(|host| setup.host_name(host).display().to_owned());
        for notice in polled.notices {
            self.notifications.set_status(notice);
        }
        changed |= polled.changed;
        if changed {
            cx.notify();
        }
    }

    /// Called for every lifecycle phase, outside `lifecycle_inner`, whose
    /// early return for a second Background must not keep a forward open.
    pub(crate) fn forwards_lifecycle(&mut self, phase: AppLifecyclePhase) {
        match phase {
            AppLifecyclePhase::Background | AppLifecyclePhase::Disconnected => {
                self.forwards.background(Instant::now(), || {
                    BackgroundTask::begin("Closing port forwards")
                });
            }
            AppLifecyclePhase::Active => self.forwards.active(),
            AppLifecyclePhase::Inactive | AppLifecyclePhase::Foreground => {}
        }
    }
}

#[cfg(test)]
#[path = "forwards_tests.rs"]
mod tests;
