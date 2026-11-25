//! T4: a finger that lands while a swipe still coasts stops the coast, sends
//! no wheel and is no tap. GPUI proves the coast was live: a landing that
//! catches one is dispatched as a scroll `Ended` (the coast closes) and then
//! a scroll `Started` (the caught touch), while a landing with no coast sends
//! no scroll at all (`gpui-pre` 0.3.12 `gestures.rs`,
//! `TouchGestureRecognizer::handle_event_at`, the `TouchPhase::Started` arm).
use super::*;
use gpui::{Context, DispatchPhase, IntoElement, ScrollEvent, Window, canvas};

/// The window, wrapped in a parent that records the phase of every scroll
/// event GPUI dispatches. It listens in the capture phase, so the terminal's
/// `stop_propagation` cannot hide one.
struct Spy {
    view: Entity<Smoke>,
    phases: Rc<RefCell<Vec<TouchPhase>>>,
}
impl Render for Spy {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let phases = self.phases.clone();
        div().relative().size_full().child(self.view.clone()).child(
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    window.on_mouse_event(move |event: &ScrollEvent, phase, _, _| {
                        if phase == DispatchPhase::Capture {
                            phases.borrow_mut().push(event.touch_phase);
                        }
                    });
                },
            )
            .absolute()
            .size_full(),
        )
    }
}

/// GPUI's `VELOCITY_ASSUME_STOPPED_GAP` (`gpui-pre` 0.3.12 `gestures.rs:513`):
/// a move or a lift this long after the move before means the finger had
/// stopped, and the lift starts no coast.
const STOPPED: Duration = Duration::from_millis(40);

/// Waits `time` on the CPU. A sleep on a loaded machine can last far longer
/// than asked (issue #180).
fn spin(time: Duration) {
    let until = Instant::now() + time;
    while Instant::now() < until {
        std::hint::spin_loop();
    }
}

fn finger(cx: &mut VisualTestContext, id: u64, phase: TouchPhase, position: Point<Pixels>) {
    cx.simulate_event(TouchEvent {
        id: TouchId(id),
        phase,
        position,
        predicted_position: None,
        force: None,
    });
}

/// One swipe's touch events, with the widest gap GPUI can have measured
/// between two of them: from just before one was sent to just after the
/// next one was handled.
#[derive(Default)]
struct Swipe {
    widest: Duration,
    previous: Option<Instant>,
}
impl Swipe {
    fn send(&mut self, cx: &mut VisualTestContext, id: u64, phase: TouchPhase, at: Point<Pixels>) {
        let before = Instant::now();
        finger(cx, id, phase, at);
        let after = Instant::now();
        if let Some(previous) = self.previous {
            self.widest = self.widest.max(after - previous);
        }
        self.previous = Some(before);
    }
}

#[gpui::test]
fn a_finger_landing_on_a_coast_stops_it_without_a_tap_or_a_wheel(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let start = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    let phases = Rc::new(RefCell::new(Vec::new()));
    cx.update(|window, cx| {
        let view = view.clone();
        let phases = phases.clone();
        window.replace_root(cx, move |_, _| Spy { view, phases });
    });
    cx.run_until_parked();
    let taps = view.read_with(cx, |this, _| this.taps);
    // GPUI measures the release speed on the wall clock (`Instant::now()` in
    // its recognizer; a test cannot set that clock) and starts no coast after
    // a gap of `STOPPED`. The moves are 4 ms apart on the CPU and the lift
    // follows the last one at once, so a swipe whose widest gap stayed under
    // `STOPPED` must coast; one that did not is a failure. A loaded machine
    // can still stall between two events (issue #180): such a swipe gets
    // another try, up to 20, and its landing finger is cancelled, as it would
    // otherwise be a tap.
    let mut caught = false;
    let mut tries = 0;
    for attempt in 0..20u64 {
        tries += 1;
        let id = 1370 + 2 * attempt;
        let landing = id + 1;
        let mut swipe = Swipe::default();
        ready(&view, cx);
        swipe.send(cx, id, TouchPhase::Started, start);
        for step in 1..=4 {
            ready(&view, cx);
            spin(Duration::from_millis(4));
            let y = step as f32 * 2. * row;
            swipe.send(cx, id, TouchPhase::Moved, start + point(px(0.), px(y)));
        }
        let lifted = start + point(px(0.), px(8. * row));
        swipe.send(cx, id, TouchPhase::Ended, lifted);
        cx.run_until_parked();
        assert!(
            !wheels(&view, cx, &wire).is_empty(),
            "the swipe scrolled nothing"
        );
        phases.borrow_mut().clear();
        ready(&view, cx);
        finger(cx, landing, TouchPhase::Started, start);
        cx.run_until_parked();
        let seen = phases.borrow().clone();
        caught = seen == [TouchPhase::Ended, TouchPhase::Started];
        assert!(caught || seen.is_empty(), "attempt {attempt}: {seen:?}");
        assert!(
            caught || swipe.widest >= STOPPED,
            "attempt {attempt}: no coast, though no gap reached {STOPPED:?} ({:?})",
            swipe.widest
        );
        let lift = if caught {
            TouchPhase::Ended
        } else {
            TouchPhase::Cancelled
        };
        ready(&view, cx);
        finger(cx, landing, lift, start);
        cx.run_until_parked();
        assert_eq!(
            wheels(&view, cx, &wire),
            [],
            "attempt {attempt}: the landing finger scrolled"
        );
        if caught {
            break;
        }
    }
    assert_eq!(view.read_with(cx, |this, _| this.taps), taps, "a tap");
    view.read_with(cx, |this, _| assert!(this.handoff.selection.is_none()));
    if caught {
        eprintln!("coast: a landing caught a live coast on try {tries}");
    } else {
        eprintln!("coast: SKIP, all {tries} swipes stalled for {STOPPED:?} or more");
    }
}
