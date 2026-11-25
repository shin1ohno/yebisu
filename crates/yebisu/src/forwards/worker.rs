//! One forward's threads. A control thread checks the host key, connects,
//! listens on this iPad's IPv4 loopback, probes the host's port once, and
//! then accepts connections, handing each to a fixed pool of relay threads.
//! `Forward::open` and `Forward::relay` can block for about 51 s, and the
//! host-key scan and the Keychain block too, so none of it runs on the UI
//! thread: the UI starts a worker, sends it commands through a bounded queue
//! and reads its latest state from a mailbox (AGENTS.md).
//!
//! The control thread pauses the forward itself when its deadline passes, so
//! a pause does not depend on the UI's timers running in the background.
use super::Reason;
use herdr_ssh::forward;
use std::{
    io,
    net::{Ipv4Addr, TcpListener, TcpStream},
    num::NonZeroU16,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

/// Relay threads per forward: at most this many carried connections wait
/// for their channel at once. A browser opens about six per host.
pub(crate) const RELAY_WORKERS: usize = 4;
/// How often the control thread accepts, reads its commands and checks the
/// connection.
const SLICE: Duration = Duration::from_millis(50);
/// Commands that may wait for the control thread.
const COMMANDS: usize = 4;

/// What a forward needs from SSH. The app's is [`SshConnector`]; tests
/// inject their own.
pub(crate) trait Connector: Send + Sync {
    /// Whether `host`'s key is pinned and still the same, as the terminal
    /// connection checks it. Blocks on the network.
    fn trusted(&self, host: &str) -> crate::Result<()>;
    /// Starts the forwarding connection to `host`. Blocks on the Keychain
    /// and the known hosts file.
    fn connect(&self, host: &str) -> Result<Box<dyn Link>, Reason>;
}

/// One forwarding connection, as [`herdr_ssh::Forward`] offers it.
pub(crate) trait Link: Send + Sync {
    /// Opens a channel to the host's `port` and closes it again at once.
    fn open(&self, port: u16) -> Result<(), forward::Error>;
    /// Carries `stream` to the host's `port`; blocks until the channel is
    /// open or has failed.
    fn relay(&self, port: u16, stream: TcpStream) -> Result<(), forward::Error>;
    /// Why the connection ended, once it has, waiting at most `timeout`.
    fn wait_end(&self, timeout: Duration) -> Option<forward::Error>;
    /// Ends the connection without blocking.
    fn stop(&self);
}

impl Link for herdr_ssh::Forward {
    fn open(&self, port: u16) -> Result<(), forward::Error> {
        herdr_ssh::Forward::open(self, forward::Destination::localhost(port)?).map(drop)
    }

    fn relay(&self, port: u16, stream: TcpStream) -> Result<(), forward::Error> {
        herdr_ssh::Forward::relay(self, forward::Destination::localhost(port)?, stream)
    }

    fn wait_end(&self, timeout: Duration) -> Option<forward::Error> {
        herdr_ssh::Forward::wait_end(self, timeout)
    }

    fn stop(&self) {
        herdr_ssh::Forward::stop(self);
    }
}

/// The app's SSH: the same host-key gate as the terminal (`core.rs`,
/// `terminal_view.rs`), then the device key and pinned host keys.
#[cfg(target_os = "ios")]
pub(crate) struct SshConnector;

#[cfg(target_os = "ios")]
impl Connector for SshConnector {
    fn trusted(&self, host: &str) -> crate::Result<()> {
        let scan = sheprd_client::ssh::scan_host_key(host)
            .map_err(|error| crate::Error::Ssh(error.to_string()))?;
        crate::hosts::authentication_gate(&scan)
    }

    fn connect(&self, host: &str) -> Result<Box<dyn Link>, Reason> {
        sheprd_client::ssh::forward_connect(host)
            .map(|forward| Box::new(forward) as Box<dyn Link>)
            .map_err(|error| Reason::from_client(&error))
    }
}

/// What the host's tests and builds other than iOS connect with: nothing.
#[cfg(not(target_os = "ios"))]
pub(crate) struct Unavailable;

#[cfg(not(target_os = "ios"))]
impl Connector for Unavailable {
    fn trusted(&self, _: &str) -> crate::Result<()> {
        Err(crate::Error::UntrustedHost)
    }

    fn connect(&self, _: &str) -> Result<Box<dyn Link>, Reason> {
        Err(Reason::ConnectTimeout)
    }
}

/// How a forward listens; [`listen`] in the app, a stand-in in tests.
pub(crate) type Bind = fn(u16) -> io::Result<TcpListener>;

/// Listens on `port` of this iPad's IPv4 loopback and nowhere else, so no
/// other device on the network can reach a forward. Port 0 lets the system
/// choose.
pub(crate) fn listen(port: u16) -> io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
}

/// Listens on `preferred`, or on a port the system chooses when another
/// program already holds it. Any other failure ends the forward.
fn bind(bind: Bind, preferred: u16) -> Result<TcpListener, Reason> {
    match bind(preferred) {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            bind(0).map_err(|_| Reason::NoLocalPort)
        }
        Err(_) => Err(Reason::NoLocalPort),
    }
}

/// What a worker is started with.
pub(crate) struct Start {
    pub(crate) host: String,
    pub(crate) remote: NonZeroU16,
    /// The local port tried first.
    pub(crate) preferred: u16,
    pub(crate) connector: Arc<dyn Connector>,
    pub(crate) bind: Bind,
}

/// What the UI asks of a running worker. Stopping is dropping its
/// [`Handle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Close the listener and the connection at this time, unless cancelled.
    PauseAt(Instant),
    CancelPause,
}

/// A worker's latest state. `Paused` and `Ended` are its last report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Report {
    Listening {
        local: u16,
        /// Nothing accepted on the host's port at the opening probe.
        nobody_yet: bool,
        /// The port tried first.
        wanted: u16,
    },
    Paused {
        last_local: u16,
    },
    Ended(Reason),
}

/// The UI's side of one worker. Dropping it stops the forward.
pub(crate) struct Handle {
    commands: SyncSender<Command>,
    /// A command the full queue refused, sent again on the next poll.
    pending: Option<Command>,
    mailbox: Arc<Mutex<Option<Report>>>,
    done: Arc<AtomicBool>,
}

impl Handle {
    /// Starts the control thread. Fails only when no thread can be started.
    pub(crate) fn spawn(start: Start) -> io::Result<Self> {
        let (commands, received) = mpsc::sync_channel(COMMANDS);
        let mailbox = Arc::new(Mutex::new(None));
        let done = Arc::new(AtomicBool::new(false));
        let name = format!("yebisu-forward-{}", start.remote);
        let control = Control {
            start,
            commands: received,
            mailbox: Arc::clone(&mailbox),
            pause_at: None,
        };
        let finished = Done(Arc::clone(&done));
        thread::Builder::new().name(name).spawn(move || {
            let _finished = finished;
            control.run();
        })?;
        Ok(Self {
            commands,
            pending: None,
            mailbox,
            done,
        })
    }

    /// The worker's report since the last call, if any.
    pub(crate) fn take(&self) -> Option<Report> {
        self.mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Sends `command` without blocking; a full queue keeps the latest one
    /// for [`Handle::resend`].
    pub(crate) fn send(&mut self, command: Command) {
        match self.commands.try_send(command) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => self.pending = None,
            Err(TrySendError::Full(command)) => self.pending = Some(command),
        }
    }

    /// Sends a command the queue refused before.
    pub(crate) fn resend(&mut self) {
        if let Some(command) = self.pending.take() {
            self.send(command);
        }
    }

    /// Set once the control thread has returned.
    pub(crate) fn done(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.done)
    }
}

/// Marks the control thread finished however it returns.
struct Done(Arc<AtomicBool>);

impl Drop for Done {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct Control {
    start: Start,
    commands: Receiver<Command>,
    mailbox: Arc<Mutex<Option<Report>>>,
    pause_at: Option<Instant>,
}

impl Control {
    fn run(mut self) {
        let remote = self.start.remote.get();
        // The host-key gate comes first: an untrusted host is never connected.
        if let Err(error) = self.start.connector.trusted(&self.start.host) {
            return self.report(Report::Ended(Reason::from_gate(&error)));
        }
        if !self.read_commands() {
            return;
        }
        let link: Arc<dyn Link> = match self.start.connector.connect(&self.start.host) {
            Ok(link) => Arc::from(link),
            Err(reason) => return self.report(Report::Ended(reason)),
        };
        let listener = match bind(self.start.bind, self.start.preferred) {
            Ok(listener) => listener,
            Err(reason) => {
                link.stop();
                return self.report(Report::Ended(reason));
            }
        };
        let Ok(local) = listener.local_addr().map(|address| address.port()) else {
            link.stop();
            return self.report(Report::Ended(Reason::NoLocalPort));
        };
        let nobody_yet = match link.open(remote) {
            Ok(()) => false,
            Err(forward::Error::PortClosed) => true,
            Err(error) => {
                link.stop();
                let reason = Reason::from_forward(&error).unwrap_or(Reason::ConnectTimeout);
                return self.report(Report::Ended(reason));
            }
        };
        if !self.read_commands() {
            link.stop();
            return;
        }
        self.report(Report::Listening {
            local,
            nobody_yet,
            wanted: self.start.preferred,
        });
        let last = self.serve(&listener, &link, local);
        // Closed before the last report, so a paused forward's port refuses
        // connections by the time the UI shows it paused.
        drop(listener);
        link.stop();
        if let Some(report) = last {
            self.report(report);
        }
    }

    /// Accepts until the forward stops, pauses or ends; the report to give
    /// once the listener is closed, none when the UI stopped it.
    fn serve(
        &mut self,
        listener: &TcpListener,
        link: &Arc<dyn Link>,
        local: u16,
    ) -> Option<Report> {
        let remote = self.start.remote.get();
        if listener.set_nonblocking(true).is_err() {
            return Some(Report::Ended(Reason::NoLocalPort));
        }
        let (relays, queue) = mpsc::sync_channel::<TcpStream>(forward::MAX_CHANNELS);
        let queue = Arc::new(Mutex::new(queue));
        for index in 0..RELAY_WORKERS {
            let link = Arc::clone(link);
            let queue = Arc::clone(&queue);
            let spawned = thread::Builder::new()
                .name(format!("yebisu-relay-{remote}-{index}"))
                .spawn(move || relay(&*link, &queue, remote));
            if spawned.is_err() {
                return Some(Report::Ended(Reason::ConnectTimeout));
            }
        }
        loop {
            accept(listener, &relays, remote);
            match self.commands.recv_timeout(SLICE) {
                Ok(command) => self.apply(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return None,
            }
            if self.pause_at.is_some_and(|at| Instant::now() >= at) {
                tracing::info!(remote, local, "port forward paused");
                return Some(Report::Paused { last_local: local });
            }
            if let Some(error) = link.wait_end(Duration::ZERO) {
                let reason = Reason::from_forward(&error).unwrap_or(Reason::ConnectionLost);
                return Some(Report::Ended(reason));
            }
        }
    }

    /// Applies every waiting command; false once the UI dropped the handle.
    fn read_commands(&mut self) -> bool {
        loop {
            match self.commands.try_recv() {
                Ok(command) => self.apply(command),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }

    fn apply(&mut self, command: Command) {
        self.pause_at = match command {
            Command::PauseAt(at) => Some(at),
            Command::CancelPause => None,
        };
    }

    fn report(&self, report: Report) {
        match report {
            Report::Listening { local, .. } => {
                tracing::info!(
                    remote = self.start.remote.get(),
                    local,
                    "port forward listening"
                );
            }
            Report::Paused { .. } => {}
            Report::Ended(reason) => tracing::warn!(
                remote = self.start.remote.get(),
                reason = reason.name(),
                "port forward ended"
            ),
        }
        *self.mailbox.lock().unwrap_or_else(PoisonError::into_inner) = Some(report);
    }
}

/// Hands each waiting connection to the relay threads; a full queue drops
/// the connection rather than starting another thread.
fn accept(listener: &TcpListener, relays: &SyncSender<TcpStream>, remote: u16) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => match relays.try_send(stream) {
                Ok(()) => {}
                Err(TrySendError::Full(stream)) => {
                    tracing::warn!(remote, "port forward busy; connection dropped");
                    drop(stream);
                }
                Err(TrySendError::Disconnected(_)) => return,
            },
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) => {
                tracing::warn!(remote, kind = ?error.kind(), "port forward accept failed");
                return;
            }
        }
    }
}

/// One relay thread: carries connections until the control thread drops
/// the queue. A failed connection is closed by `relay` and not retried.
fn relay(link: &dyn Link, queue: &Mutex<Receiver<TcpStream>>, remote: u16) {
    loop {
        let next = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
        let Ok(stream) = next else {
            return;
        };
        if let Err(error) = link.relay(remote, stream) {
            let reason = Reason::from_forward(&error).map(Reason::name);
            tracing::debug!(remote, reason, "carried connection failed");
        }
    }
}
