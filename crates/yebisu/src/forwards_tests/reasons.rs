//! Each typed failure's words, and that nothing a host wrote reaches them.
use super::*;
use herdr_ssh::Failure;

const PRO: &str = "pro-dev";

#[test]
fn table() {
    let text =
        |error: forward::Error| Reason::from_forward(&error).map(|reason| reason.text(PRO, 3000));
    let prohibited = "pro-dev does not allow port forwarding for this iPad's key. Remove restrict from this iPad's line in ~/.ssh/authorized_keys on pro-dev.";
    let timeout = "SSH did not connect in time; check host trust and authentication";
    let untrusted = "host is untrusted or its key has changed; authentication refused";
    let lost = "SSH connection to pro-dev ended; check the network";
    let refused = "pro-dev refused to forward port 3000";
    for (error, expected) in [
        (forward::Error::Prohibited, Some(prohibited)),
        (forward::Error::Timeout, Some(timeout)),
        (forward::Error::Connect(Failure::Timeout), Some(timeout)),
        (forward::Error::Connect(Failure::Unreachable), Some(timeout)),
        (forward::Error::Connect(Failure::Protocol), Some(timeout)),
        (forward::Error::Connect(Failure::Cancelled), Some(timeout)),
        (
            forward::Error::Connect(Failure::Auth),
            Some("pro-dev did not accept this iPad's key"),
        ),
        (
            forward::Error::Connect(Failure::HostKeyUnknown),
            Some(untrusted),
        ),
        (
            forward::Error::Connect(Failure::HostKeyChanged),
            Some(untrusted),
        ),
        (forward::Error::ConnectionLost(Failure::Timeout), Some(lost)),
        (
            forward::Error::ConnectionLost(Failure::Unreachable),
            Some(lost),
        ),
        (forward::Error::Refused, Some(refused)),
        (forward::Error::Busy, Some(refused)),
        (forward::Error::InvalidPort, Some(refused)),
        (forward::Error::NotLoopback, Some(refused)),
        (
            forward::Error::Io(io::ErrorKind::BrokenPipe.into()),
            Some(timeout),
        ),
        (forward::Error::PortClosed, None),
        (forward::Error::Stopped, None),
    ] {
        let name = format!("{error:?}");
        assert_eq!(text(error).as_deref(), expected, "{name}");
    }
    assert_eq!(untrusted, crate::Error::UntrustedHost.to_string());
    assert_eq!(
        Reason::NoLocalPort.text(PRO, 3000),
        "no local port is free to forward to"
    );
    assert_eq!(
        Reason::from_gate(&crate::Error::UntrustedHost),
        Reason::Untrusted
    );
    assert_eq!(
        Reason::from_gate(&crate::Error::Ssh("x".into())),
        Reason::ConnectTimeout
    );
}

#[test]
fn port_closed_is_listening_with_note() {
    let (mut forwards, _) = forwards(Fake {
        probe: || Err(forward::Error::PortClosed),
        ..Fake::default()
    });
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    let notices = poll_until(&mut forwards, |forwards| listening(forwards, remote));
    assert_eq!(
        state(&forwards, remote),
        Some(State::Listening {
            local: remote,
            nobody_yet: true
        })
    );
    assert_eq!(
        notices,
        [format!(
            "Forwarding port {remote} to 127.0.0.1:{remote} · nothing listens on {remote} on pro-dev yet"
        )]
    );
}

#[test]
fn prohibited_at_the_probe_ends() {
    let (mut forwards, seen) = forwards(Fake {
        probe: || Err(forward::Error::Prohibited),
        ..Fake::default()
    });
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    let notices = poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    });
    assert_eq!(
        state(&forwards, remote),
        Some(State::Ended(Reason::Prohibited))
    );
    assert_eq!(
        notices,
        [format!(
            "Port {remote} forward ended: pro-dev does not allow port forwarding for this iPad's key. Remove restrict from this iPad's line in ~/.ssh/authorized_keys on pro-dev."
        )]
    );
    assert_eq!(Seen::count(&seen.stops), 1, "the connection is closed");
    assert!(
        refuses_soon(remote),
        "nothing listens after the forward ended"
    );
}

#[test]
fn connection_lost_ends_and_closes() {
    let (mut forwards, seen, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    seen.end_with(|| forward::Error::ConnectionLost(Failure::Unreachable));
    let notices = poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    });
    assert_eq!(
        state(&forwards, remote),
        Some(State::Ended(Reason::ConnectionLost))
    );
    assert_eq!(
        notices,
        [format!(
            "Port {remote} forward ended: SSH connection to pro-dev ended; check the network"
        )]
    );
    assert!(refuses_soon(local), "the listener closed");
}

#[test]
fn connect_refused_ends() {
    let (mut forwards, seen) = forwards(Fake {
        connect: Some(Reason::KeyRefused),
        ..Fake::default()
    });
    ok(forwards.start(HOST, port(3000)));
    let notices = poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, 3000), Some(State::Ended(_)))
    });
    assert_eq!(
        notices,
        ["Port 3000 forward ended: pro-dev did not accept this iPad's key"]
    );
    assert_eq!(Seen::count(&seen.opens), 0);
}

/// A host's text inside an error never reaches a notice, a row, a label,
/// `qa.state` names or a log line.
#[test]
fn no_host_text() {
    let logs = captured_logs();
    let mut shown = Vec::new();
    // Text inside the probe's error.
    let (mut probed, _) = forwards(Fake {
        probe: || Err(forward::Error::Io(io::Error::other("EVIL from the host"))),
        ..Fake::default()
    });
    let remote = free_port();
    ok(probed.start(HOST, port(remote)));
    shown.extend(poll_until(&mut probed, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    }));
    // Text inside the host-key scan's error.
    let (mut scanned, _) = forwards(Fake {
        gate: Gate::Unreachable,
        ..Fake::default()
    });
    ok(scanned.start(HOST, port(remote)));
    shown.extend(poll_until(&mut scanned, |forwards| {
        matches!(state(forwards, remote), Some(State::Ended(_)))
    }));
    // Text inside a lost connection's error.
    let (mut lost, seen, lost_remote) = listening_forward(Fake::default());
    seen.end_with(|| forward::Error::Io(io::Error::other("EVIL after listening")));
    shown.extend(poll_until(&mut lost, |forwards| {
        matches!(state(forwards, lost_remote), Some(State::Ended(_)))
    }));
    for list in [&probed, &scanned, &lost] {
        for forward in list.entries() {
            shown.push(forward.label());
            shown.push(forward.state.name().to_owned());
            if let State::Ended(reason) = forward.state {
                shown.push(reason.text(PRO, remote));
                shown.push(reason.name().to_owned());
            }
        }
    }
    assert_eq!(
        shown
            .iter()
            .filter(|text| text.contains("forward ended: "))
            .count(),
        3
    );
    let logs = logs.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert!(
        !logs.is_empty(),
        "the capture saw the forwards' logs: {logs:?}"
    );
    for text in shown.iter().chain(&logs) {
        assert!(!text.contains("EVIL"), "host text leaked: {text}");
    }
}
