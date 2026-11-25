//! The list's rules: what the sheet refuses, how many may exist, what an
//! ended forward allows, and what a saved host's removal stops.
use super::*;
use gpui::{AppContext, TestAppContext};

#[test]
fn port_text() {
    for text in [
        "0", "65536", "3e3", " ", "", "-1", "+80", "８０", "80 80", "0x50",
    ] {
        assert_eq!(parse_port(text), Err(InputError::Port), "{text:?}");
    }
    assert_eq!(
        InputError::Port.to_string(),
        "Enter a port number from 1 to 65535."
    );
    for (text, number) in [("1", 1), (" 3000 ", 3000), ("65535", 65535), ("0080", 80)] {
        assert_eq!(parse_port(text), Ok(port(number)), "{text:?}");
    }
}

#[test]
fn duplicate_refused() {
    let (mut forwards, _) = forwards(Fake::default());
    ok(forwards.start(HOST, port(3000)));
    assert_eq!(
        forwards.start(HOST, port(3000)),
        Err(InputError::Duplicate(3000))
    );
    assert_eq!(
        InputError::Duplicate(3000).to_string(),
        "Port 3000 is already forwarded from this host."
    );
    // The same port of another host is another forward.
    ok(forwards.start("me@other-host", port(3000)));
    assert_eq!(forwards.entries().len(), 2);
    assert_eq!(forwards.for_host(HOST).count(), 1);
}

#[test]
fn ended_is_replaced() {
    let (mut forwards, seen) = forwards(Fake {
        probe: || Err(forward::Error::Prohibited),
        ..Fake::default()
    });
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    });
    ok(forwards.start(HOST, port(remote)));
    assert_eq!(state(&forwards, remote), Some(State::Connecting));
    assert_eq!(
        forwards.for_host(HOST).count(),
        1,
        "the ended row is replaced"
    );
    poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    });
    assert_eq!(Seen::count(&seen.connects), 2);
}

#[test]
fn limit_32() {
    // Every forward ends at once; ended rows still count, as in Sheprd.
    let (mut forwards, _) = forwards(Fake {
        gate: Gate::Untrusted,
        ..Fake::default()
    });
    for number in 1..=32 {
        ok(forwards.start(HOST, port(number)));
    }
    assert_eq!(forwards.start(HOST, port(33)), Err(InputError::Limit(32)));
    assert_eq!(
        InputError::Limit(MAX_FORWARDS).to_string(),
        "At most 32 ports can be forwarded at once."
    );
    // Hosts share the limit.
    assert_eq!(
        forwards.start("me@other-host", port(33)),
        Err(InputError::Limit(32))
    );
    assert!(forwards.dismiss(HOST, 1));
    ok(forwards.start(HOST, port(33)));
}

#[test]
fn url_label_and_first_open_hint() {
    let (mut forwards, _, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    let forward = ok(forwards.get(HOST, remote).ok_or("no forward"));
    assert_eq!(
        forward.label(),
        format!("Port {remote} forwarded to 127.0.0.1:{local}")
    );
    assert_eq!(
        forwards.open(HOST, remote),
        Some((format!("http://127.0.0.1:{local}/"), Some(OPEN_HINT)))
    );
    assert_eq!(
        OPEN_HINT,
        "Keep yebisu on screen beside Safari, or the forward pauses."
    );
    assert_eq!(
        forwards.open(HOST, remote),
        Some((format!("http://127.0.0.1:{local}/"), None)),
        "the hint shows once per process"
    );
    assert_eq!(forwards.open(HOST, 1), None);
}

#[test]
fn other_state_labels() {
    let (mut forwards, _) = forwards(Fake {
        probe: || Err(forward::Error::Prohibited),
        ..Fake::default()
    });
    ok(forwards.start(HOST, port(3000)));
    let label = |forwards: &Forwards| ok(forwards.get(HOST, 3000).ok_or("no forward")).label();
    assert_eq!(label(&forwards), "Port 3000 forward connecting");
    assert_eq!(ok(forwards.get(HOST, 3000).ok_or("no forward")).url(), None);
    poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, 3000), Some(State::Ended(_)))
    });
    assert_eq!(label(&forwards), "Port 3000 forward ended");
    assert_eq!(forwards.open(HOST, 3000), None);
}

#[test]
fn stop_removes_the_row() {
    let (mut forwards, _, remote) = listening_forward(Fake::default());
    assert!(forwards.stop(HOST, remote));
    assert!(!forwards.stop(HOST, remote));
    assert!(forwards.entries().is_empty());
}

/// Through the app's own poll, so dropping the call there fails too.
#[gpui::test]
fn retain_hosts_stops_removed(cx: &mut TestAppContext) {
    let fake = Fake::default();
    let seen = Arc::clone(&fake.seen);
    let other = "me@other-host";
    let remote = free_port();
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.forwards = Forwards::with(Arc::new(fake), worker::listen);
        this.setup.hosts = vec![HOST.to_owned(), other.to_owned()];
        ok(this.forwards.start(HOST, port(remote)));
        ok(this.forwards.start(other, port(remote)));
        let deadline = Instant::now() + PATIENCE;
        while this
            .forwards
            .entries()
            .iter()
            .any(|forward| !forward.state.live())
            || this
                .forwards
                .entries()
                .iter()
                .any(|forward| forward.state == State::Connecting)
        {
            assert!(Instant::now() < deadline, "the forwards did not open");
            this.poll_forwards(cx);
            thread::sleep(Duration::from_millis(5));
        }
        // The host's destination is gone from the saved list.
        this.setup.hosts = vec![other.to_owned()];
        this.poll_forwards(cx);
        assert!(
            this.forwards.get(HOST, remote).is_none(),
            "its forward stops"
        );
        assert!(this.forwards.get(other, remote).is_some(), "others stay");
    });
    let deadline = Instant::now() + PATIENCE;
    while Seen::count(&seen.stops) == 0 {
        assert!(
            Instant::now() < deadline,
            "the removed host's link was not stopped"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(Seen::count(&seen.stops), 1);
}

/// What a forward reports reaches the status line through the app's poll.
#[gpui::test]
fn notices_reach_the_status_line(cx: &mut TestAppContext) {
    let remote = free_port();
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.forwards = Forwards::with(Arc::new(Fake::default()), worker::listen);
        this.setup.hosts = vec![HOST.to_owned()];
        ok(this.forwards.start(HOST, port(remote)));
        let deadline = Instant::now() + PATIENCE;
        while !listening(&this.forwards, remote) {
            assert!(Instant::now() < deadline, "the forward did not open");
            this.poll_forwards(cx);
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            this.notifications.visible_status(),
            format!("Forwarding port {remote} to 127.0.0.1:{remote}")
        );
    });
}
