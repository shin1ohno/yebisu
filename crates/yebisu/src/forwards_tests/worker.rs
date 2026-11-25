//! The worker: where it listens, which port it takes, what it calls and on
//! which thread, and how many relays run at once.
use super::*;
use std::net::{Ipv4Addr, UdpSocket};

/// This machine's address on its default route, if it has one: a UDP
/// "connect" only picks the source address and sends nothing.
fn outside_address() -> Option<std::net::IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

#[test]
fn binds_loopback_only() {
    let listener = ok(worker::listen(0));
    assert_eq!(ok(listener.local_addr()).ip(), Ipv4Addr::LOCALHOST);
    let (forwards, _, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    assert!(accepts(local), "the forward accepts on 127.0.0.1");
    if let Some(ip) = outside_address() {
        let reached = TcpStream::connect_timeout(
            &std::net::SocketAddr::new(ip, local),
            Duration::from_millis(500),
        );
        assert!(reached.is_err(), "{ip}:{local} must not reach the forward");
    }
}

#[test]
fn prefers_same_port() {
    let (mut forwards, _) = forwards(Fake::default());
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    let notices = poll_until(&mut forwards, |forwards| listening(forwards, remote));
    assert_eq!(local(&forwards, remote), remote);
    assert_eq!(
        notices,
        [format!("Forwarding port {remote} to 127.0.0.1:{remote}")]
    );
}

static ASKED: Mutex<Vec<u16>> = Mutex::new(Vec::new());

/// Records the ports asked for, and listens where the system chooses.
fn recording_bind(port: u16) -> io::Result<TcpListener> {
    ASKED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(port);
    worker::listen(0)
}

#[test]
fn privileged_moves_up() {
    assert_eq!(sheprd_client::preferred_local_port(port(80)), 10080);
    assert_eq!(sheprd_client::preferred_local_port(port(3000)), 3000);
    let mut forwards = Forwards::with(Arc::new(Fake::default()), recording_bind);
    ok(forwards.start(HOST, port(80)));
    poll_until(&mut forwards, |forwards| listening(forwards, 80));
    let asked = ASKED.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert_eq!(asked.first(), Some(&10080), "80 is tried as 10080 first");
}

#[test]
fn falls_back_when_in_use() {
    let held = ok(worker::listen(0));
    let remote = ok(held.local_addr()).port();
    let (mut forwards, _) = forwards(Fake::default());
    ok(forwards.start(HOST, port(remote)));
    let notices = poll_until(&mut forwards, |forwards| listening(forwards, remote));
    let local = local(&forwards, remote);
    assert_ne!(local, remote, "the held port is not taken twice");
    assert!(accepts(local));
    assert_eq!(
        notices,
        [format!(
            "Port {remote} now forwards to 127.0.0.1:{local} ({remote} is in use on this iPad)"
        )]
    );
    drop(held);
}

/// Refuses the port asked for with a failure other than "in use", and
/// would listen if the worker fell back anyway.
fn denied_bind(port: u16) -> io::Result<TcpListener> {
    if port == 0 {
        worker::listen(0)
    } else {
        Err(io::ErrorKind::PermissionDenied.into())
    }
}

#[test]
fn other_bind_error_ends() {
    let mut forwards = Forwards::with(Arc::new(Fake::default()), denied_bind);
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    let notices = poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    });
    assert_eq!(
        state(&forwards, remote),
        Some(State::Ended(Reason::NoLocalPort))
    );
    assert_eq!(
        notices,
        [format!(
            "Port {remote} forward ended: no local port is free to forward to"
        )]
    );
}

#[test]
fn untrusted_never_connects() {
    let (mut forwards, seen) = forwards(Fake {
        gate: Gate::Untrusted,
        ..Fake::default()
    });
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    let notices = poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    });
    assert_eq!(
        state(&forwards, remote),
        Some(State::Ended(Reason::Untrusted))
    );
    assert_eq!(Seen::count(&seen.trusted), 1);
    assert_eq!(
        Seen::count(&seen.connects),
        0,
        "an untrusted host is never connected"
    );
    assert_eq!(Seen::count(&seen.opens), 0);
    assert_eq!(
        notices,
        [format!(
            "Port {remote} forward ended: host is untrusted or its key has changed; authentication refused"
        )]
    );
}

#[test]
fn calls_off_ui_thread() {
    let ui = thread::current().id();
    let (mut forwards, seen, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    let stream = ok(TcpStream::connect((Ipv4Addr::LOCALHOST, local)));
    let deadline = Instant::now() + PATIENCE;
    while Seen::count(&seen.relays) == 0 {
        assert!(Instant::now() < deadline, "the connection was not relayed");
        thread::sleep(Duration::from_millis(5));
    }
    drop(stream);
    assert!(forwards.stop(HOST, remote));
    let threads = seen
        .threads
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    // trusted, connect, open and one relay at least.
    assert!(threads.len() >= 4, "{threads:?}");
    assert!(
        threads.iter().all(|thread| *thread != ui),
        "no SSH call runs on the UI thread"
    );
}

#[test]
fn relay_pool_bounded() {
    let (mut forwards, seen, remote) = listening_forward(Fake {
        hold_relays: true,
        ..Fake::default()
    });
    let local = local(&forwards, remote);
    let streams: Vec<_> = (0..10)
        .map(|_| ok(TcpStream::connect((Ipv4Addr::LOCALHOST, local))))
        .collect();
    let deadline = Instant::now() + PATIENCE;
    while Seen::count(&seen.relaying) < worker::RELAY_WORKERS {
        assert!(
            Instant::now() < deadline,
            "the relay workers did not all start"
        );
        thread::sleep(Duration::from_millis(5));
    }
    // Give a thread-per-connection worker time to show itself.
    thread::sleep(Duration::from_millis(300));
    assert_eq!(
        Seen::count(&seen.most_relaying),
        worker::RELAY_WORKERS,
        "never more relays at once than the pool"
    );
    seen.release.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + PATIENCE;
    while Seen::count(&seen.relays) < streams.len() {
        assert!(
            Instant::now() < deadline,
            "queued connections were not relayed"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(Seen::count(&seen.most_relaying) <= worker::RELAY_WORKERS);
    drop(streams);
    assert!(forwards.stop(HOST, remote));
}

#[test]
fn stop_closes_the_listener() {
    let (mut forwards, seen, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    assert!(forwards.stop(HOST, remote));
    assert!(forwards.get(HOST, remote).is_none());
    let deadline = Instant::now() + PATIENCE;
    while accepts(local) || Seen::count(&seen.stops) == 0 {
        assert!(
            Instant::now() < deadline,
            "the stopped forward still listens"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
