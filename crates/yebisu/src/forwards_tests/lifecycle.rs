//! Off screen and back: the grace, the pause, the resume on the same port,
//! and what an ended forward does meanwhile.
use super::*;
use gpui::{AppContext, TestAppContext};

/// A background task whose release the test can see.
struct Task(Arc<AtomicBool>);

impl Drop for Task {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn granted() -> (Arc<AtomicBool>, impl FnOnce() -> Option<Task>) {
    let released = Arc::new(AtomicBool::new(false));
    let task = Task(Arc::clone(&released));
    (released, move || Some(task))
}

fn paused(forwards: &Forwards, remote: u16) -> bool {
    state(forwards, remote) == Some(State::Paused)
}

/// Polls for `period`, failing if the forward is ever not listening.
fn stays_listening(forwards: &mut Forwards, remote: u16, period: Duration) {
    let until = Instant::now() + period;
    while Instant::now() < until {
        forwards.poll(name);
        assert!(listening(forwards, remote), "{:?}", state(forwards, remote));
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn states_walk_and_return() {
    let (mut forwards, seen) = forwards(Fake::default());
    let remote = free_port();
    ok(forwards.start(HOST, port(remote)));
    assert_eq!(state(&forwards, remote), Some(State::Connecting));
    poll_until(&mut forwards, |forwards| listening(forwards, remote));
    // No background time: it pauses at once.
    forwards.background(Instant::now(), || None::<Task>);
    poll_until(&mut forwards, |forwards| paused(forwards, remote));
    assert!(refuses_soon(remote), "a paused forward does not listen");
    forwards.active();
    assert_eq!(state(&forwards, remote), Some(State::Connecting));
    poll_until(&mut forwards, |forwards| listening(forwards, remote));
    assert_eq!(local(&forwards, remote), remote);
    assert!(accepts(remote));
    assert_eq!(Seen::count(&seen.connects), 2, "a resume connects again");
}

#[test]
fn pauses_after_grace() {
    let (mut forwards, _, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    let (released, begin) = granted();
    let left = Instant::now();
    forwards.background(left, begin);
    // Still forwarding within the grace.
    stays_listening(&mut forwards, remote, PAUSE_GRACE / 2);
    assert!(accepts(local));
    poll_until(&mut forwards, |forwards| paused(forwards, remote));
    assert!(left.elapsed() >= PAUSE_GRACE, "paused only after the grace");
    assert!(
        refuses_soon(local),
        "the paused forward's port refuses connections"
    );
    assert_eq!(
        ok(forwards.get(HOST, remote).ok_or("gone")).last_local,
        Some(local)
    );
    let deadline = Instant::now() + PATIENCE;
    while !released.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "the background task was not released"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn no_background_time_pauses_at_once() {
    let (mut forwards, _, remote) = listening_forward(Fake::default());
    let left = Instant::now();
    forwards.background(left, || None::<Task>);
    poll_until(&mut forwards, |forwards| paused(forwards, remote));
    assert!(left.elapsed() < PAUSE_GRACE);
}

#[test]
fn active_within_grace_keeps_listening() {
    let (mut forwards, seen, remote) = listening_forward(Fake::default());
    let local = local(&forwards, remote);
    let (released, begin) = granted();
    forwards.background(Instant::now(), begin);
    stays_listening(&mut forwards, remote, Duration::from_millis(500));
    forwards.active();
    // Past the grace: nothing stopped, nothing reconnected.
    stays_listening(&mut forwards, remote, PAUSE_GRACE + Duration::from_secs(1));
    assert_eq!(super::local(&forwards, remote), local);
    assert!(accepts(local));
    assert_eq!(
        Seen::count(&seen.connects),
        1,
        "the forward was never closed"
    );
    assert_eq!(Seen::count(&seen.stops), 0);
    assert!(
        released.load(Ordering::SeqCst),
        "the background task ends on return"
    );
}

#[test]
fn resume_reuses_last_port() {
    // The remote port's own number is taken here, so the forward listens
    // elsewhere; the resume must come back there, not to the preferred port.
    let held = ok(worker::listen(0));
    let remote = ok(held.local_addr()).port();
    let (mut forwards, _) = forwards(Fake::default());
    ok(forwards.start(HOST, port(remote)));
    poll_until(&mut forwards, |forwards| listening(forwards, remote));
    let first = local(&forwards, remote);
    assert_ne!(first, remote);
    forwards.background(Instant::now(), || None::<Task>);
    poll_until(&mut forwards, |forwards| paused(forwards, remote));
    forwards.active();
    let notices = poll_until(&mut forwards, |forwards| listening(forwards, remote));
    assert_eq!(
        local(&forwards, remote),
        first,
        "the previous local port first"
    );
    assert_eq!(
        notices,
        [format!("Forwarding port {remote} to 127.0.0.1:{first}")]
    );
    drop(held);
}

#[test]
fn resume_moves_when_the_last_port_is_taken() {
    let (mut forwards, _, remote) = listening_forward(Fake::default());
    forwards.background(Instant::now(), || None::<Task>);
    poll_until(&mut forwards, |forwards| paused(forwards, remote));
    // Another program on the iPad takes the port meanwhile.
    let taken = ok(worker::listen(remote));
    forwards.active();
    let notices = poll_until(&mut forwards, |forwards| listening(forwards, remote));
    let now = local(&forwards, remote);
    assert_ne!(now, remote);
    assert_eq!(
        notices,
        [format!(
            "Port {remote} now forwards to 127.0.0.1:{now} ({remote} is in use on this iPad)"
        )]
    );
    drop(taken);
}

#[test]
fn ended_stays_ended_on_active() {
    let (mut forwards, seen) = forwards(Fake {
        probe: || Err(forward::Error::Prohibited),
        ..Fake::default()
    });
    ok(forwards.start(HOST, port(3000)));
    poll_until(&mut forwards, |forwards| {
        matches!(state(forwards, 3000), Some(State::Ended(_)))
    });
    forwards.background(Instant::now(), || None::<Task>);
    forwards.active();
    forwards.poll(name);
    assert_eq!(
        state(&forwards, 3000),
        Some(State::Ended(Reason::Prohibited))
    );
    assert_eq!(
        Seen::count(&seen.connects),
        1,
        "nothing restarts an ended forward"
    );
}

#[test]
fn nothing_live_asks_for_no_background_time() {
    let (mut forwards, _) = forwards(Fake::default());
    let mut asked = false;
    forwards.background(Instant::now(), || {
        asked = true;
        None::<Task>
    });
    assert!(!asked);
}

/// Through the app's lifecycle, with the terminal already suspended: the
/// early return in `lifecycle_inner` for a second Background must not keep
/// the forward open.
#[gpui::test]
fn second_background_still_pauses(cx: &mut TestAppContext) {
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
        this.terminal.reconnect.suspended = true;
        this.lifecycle(AppLifecyclePhase::Background, cx);
        let deadline = Instant::now() + PAUSE_GRACE + PATIENCE;
        while !paused(&this.forwards, remote) {
            assert!(
                Instant::now() < deadline,
                "the forward stayed open off screen"
            );
            this.poll_forwards(cx);
            thread::sleep(Duration::from_millis(20));
        }
        assert!(refuses_soon(remote));
        this.lifecycle(AppLifecyclePhase::Active, cx);
        let deadline = Instant::now() + PATIENCE;
        while !listening(&this.forwards, remote) {
            assert!(Instant::now() < deadline, "the forward did not resume");
            this.poll_forwards(cx);
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(local(&this.forwards, remote), remote);
    });
}
