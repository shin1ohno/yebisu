//! Port forward tests: a fake SSH connector and link driven by real loopback
//! listeners, so the worker's threads, binds and accepts are exercised as
//! the app runs them. Each topic has its own file.

use super::*;
use herdr_ssh::forward;
use std::{
    io,
    net::{TcpListener, TcpStream},
    sync::{
        Mutex, OnceLock, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread::ThreadId,
    time::Duration,
};
use worker::{Connector, Link};

#[path = "forwards_tests/lifecycle.rs"]
mod lifecycle;
#[path = "forwards_tests/model.rs"]
mod model;
#[path = "forwards_tests/reasons.rs"]
mod reasons;
#[path = "forwards_tests/scan.rs"]
mod scan;
#[path = "forwards_tests/worker.rs"]
mod worker_tests;

/// The value of a step a test cannot go on without.
#[track_caller]
pub(super) fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("{error:?}"))
}

/// The background task in host tests: always granted. iOS test builds use
/// herdr-uikit's own.
#[cfg(not(target_os = "ios"))]
pub(super) struct BackgroundTask;

#[cfg(not(target_os = "ios"))]
impl BackgroundTask {
    pub(super) fn begin(_: &str) -> Option<Self> {
        Some(Self)
    }
}

/// How long a test waits for a worker before failing.
pub(super) const PATIENCE: Duration = Duration::from_secs(10);
pub(super) const HOST: &str = "me@pro-dev";

/// What the host-key gate answers.
#[derive(Clone, Copy)]
pub(super) enum Gate {
    Trusted,
    Untrusted,
    /// The scan failed with text the host could have written.
    Unreachable,
}

/// What the fake SSH saw, from every thread.
#[derive(Default)]
pub(super) struct Seen {
    pub(super) trusted: AtomicUsize,
    pub(super) connects: AtomicUsize,
    pub(super) opens: AtomicUsize,
    pub(super) relays: AtomicUsize,
    pub(super) stops: AtomicUsize,
    /// Relays running now, and the most at once.
    pub(super) relaying: AtomicUsize,
    pub(super) most_relaying: AtomicUsize,
    /// The threads each call ran on.
    pub(super) threads: Mutex<Vec<ThreadId>>,
    /// What the next `wait_end` reports, once.
    pub(super) end: Mutex<Option<fn() -> forward::Error>>,
    /// Relays block until this is set, when `hold_relays` is.
    pub(super) release: AtomicBool,
}

impl Seen {
    fn called(&self, count: &AtomicUsize) {
        count.fetch_add(1, Ordering::SeqCst);
        self.threads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(thread::current().id());
    }

    pub(super) fn count(count: &AtomicUsize) -> usize {
        count.load(Ordering::SeqCst)
    }

    /// Makes the forward's connection end with `error` at its next check.
    pub(super) fn end_with(&self, error: fn() -> forward::Error) {
        *self.end.lock().unwrap_or_else(PoisonError::into_inner) = Some(error);
    }
}

/// A scripted SSH: the gate, the connect, the opening probe and the relays.
pub(super) struct Fake {
    pub(super) gate: Gate,
    pub(super) connect: Option<Reason>,
    pub(super) probe: fn() -> Result<(), forward::Error>,
    pub(super) hold_relays: bool,
    pub(super) seen: Arc<Seen>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            gate: Gate::Trusted,
            connect: None,
            probe: || Ok(()),
            hold_relays: false,
            seen: Arc::default(),
        }
    }
}

impl Connector for Fake {
    fn trusted(&self, _: &str) -> crate::Result<()> {
        self.seen.called(&self.seen.trusted);
        match self.gate {
            Gate::Trusted => Ok(()),
            Gate::Untrusted => Err(crate::Error::UntrustedHost),
            Gate::Unreachable => Err(crate::Error::Ssh("EVIL scan text from the host".into())),
        }
    }

    fn connect(&self, _: &str) -> Result<Box<dyn Link>, Reason> {
        self.seen.called(&self.seen.connects);
        if let Some(reason) = self.connect {
            return Err(reason);
        }
        Ok(Box::new(FakeLink {
            probe: self.probe,
            hold_relays: self.hold_relays,
            seen: Arc::clone(&self.seen),
        }))
    }
}

struct FakeLink {
    probe: fn() -> Result<(), forward::Error>,
    hold_relays: bool,
    seen: Arc<Seen>,
}

impl Link for FakeLink {
    fn open(&self, _: u16) -> Result<(), forward::Error> {
        self.seen.called(&self.seen.opens);
        (self.probe)()
    }

    fn relay(&self, _: u16, stream: TcpStream) -> Result<(), forward::Error> {
        self.seen.called(&self.seen.relays);
        let now = self.seen.relaying.fetch_add(1, Ordering::SeqCst) + 1;
        self.seen.most_relaying.fetch_max(now, Ordering::SeqCst);
        while self.hold_relays
            && !self.seen.release.load(Ordering::SeqCst)
            && Seen::count(&self.seen.stops) == 0
        {
            thread::sleep(Duration::from_millis(5));
        }
        self.seen.relaying.fetch_sub(1, Ordering::SeqCst);
        drop(stream);
        Ok(())
    }

    fn wait_end(&self, _: Duration) -> Option<forward::Error> {
        let end = self
            .seen
            .end
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        end.map(|error| error())
    }

    fn stop(&self) {
        self.seen.stops.fetch_add(1, Ordering::SeqCst);
    }
}

/// Forwards over `fake`, listening on the real loopback.
pub(super) fn forwards(fake: Fake) -> (Forwards, Arc<Seen>) {
    let seen = Arc::clone(&fake.seen);
    (Forwards::with(Arc::new(fake), worker::listen), seen)
}

/// The host's display name, as the app passes it.
pub(super) fn name(host: &str) -> String {
    crate::hosts::HostName::new(host).display().to_owned()
}

/// Polls until `done` holds, collecting every notice; fails after
/// [`PATIENCE`].
pub(super) fn poll_until(forwards: &mut Forwards, done: impl Fn(&Forwards) -> bool) -> Vec<String> {
    let deadline = Instant::now() + PATIENCE;
    let mut notices = Vec::new();
    loop {
        notices.extend(forwards.poll(name).notices);
        if done(forwards) {
            return notices;
        }
        assert!(
            Instant::now() < deadline,
            "forward did not get there in time"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

/// The state of `host`'s `remote` forward.
pub(super) fn state(forwards: &Forwards, remote: u16) -> Option<State> {
    forwards.get(HOST, remote).map(|forward| forward.state)
}

pub(super) fn listening(forwards: &Forwards, remote: u16) -> bool {
    matches!(state(forwards, remote), Some(State::Listening { .. }))
}

pub(super) fn local(forwards: &Forwards, remote: u16) -> u16 {
    match state(forwards, remote) {
        Some(State::Listening { local, .. }) => local,
        other => panic!("not listening: {other:?}"),
    }
}

pub(super) fn port(number: u16) -> NonZeroU16 {
    NonZeroU16::new(number).unwrap_or_else(|| panic!("port 0"))
}

/// A loopback port nothing listens on now (the system's ephemeral range,
/// above 1023, so it is its own preferred local port).
pub(super) fn free_port() -> u16 {
    ok(ok(worker::listen(0)).local_addr()).port()
}

/// Whether a TCP connection to `port` on this machine's loopback is
/// accepted.
pub(super) fn accepts(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(500),
    )
    .is_ok()
}

/// Whether a connection to `port` on this machine's loopback is refused:
/// nothing listens there. A timeout is not a refusal, since a listener that
/// nobody accepts on times out once its queue is full.
pub(super) fn refused(port: u16) -> bool {
    matches!(
        TcpStream::connect_timeout(
            &std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
            Duration::from_millis(500),
        ),
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused
    )
}

/// Whether connections to `port` are refused within [`PATIENCE`] of a
/// forward closing its listener. Under a full parallel `cargo test` on a
/// loaded machine the freed port was seen to answer once right after the
/// close, from something outside the forward (it passes alone 5 of 5), so
/// this asks again until the deadline. A listener left open still fails.
pub(super) fn refuses_soon(port: u16) -> bool {
    let deadline = Instant::now() + PATIENCE;
    while !refused(port) {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
    true
}

/// Starts a forward of a free port and waits until it listens.
pub(super) fn listening_forward(fake: Fake) -> (Forwards, Arc<Seen>, u16) {
    let (mut forwards, seen) = forwards(fake);
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    poll_until(&mut forwards, |forwards| listening(forwards, remote));
    (forwards, seen, remote)
}

/// Every log line the forwards wrote while the tests ran, captured by a
/// process-wide subscriber installed on first use (worker threads log
/// too, and a thread-local subscriber would miss them).
pub(super) fn captured_logs() -> &'static Mutex<Vec<String>> {
    static LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        let _ = tracing::subscriber::set_global_default(LogCapture);
        Mutex::new(Vec::new())
    })
}

struct LogCapture;

struct Line(String);

impl tracing::field::Visit for Line {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!(" {}={value:?}", field.name()));
    }
}

impl tracing::Subscriber for LogCapture {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().starts_with("yebisu::forwards")
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }
        let mut line = Line(event.metadata().target().to_owned());
        event.record(&mut line);
        captured_logs()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
