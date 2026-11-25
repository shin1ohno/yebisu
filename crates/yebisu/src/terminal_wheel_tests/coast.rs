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

fn finger(cx: &mut VisualTestContext, id: u64, phase: TouchPhase, position: Point<Pixels>) {
    cx.simulate_event(TouchEvent {
        id: TouchId(id),
        phase,
        position,
        predicted_position: None,
        force: None,
    });
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
    // GPUI measures the release speed on the wall clock and starts no coast
    // when the lift comes more than 40 ms after the last move
    // (`VELOCITY_ASSUME_STOPPED_GAP`), which a loaded machine can cause, so
    // the swipe gets a few tries. A try without a coast cancels its landing
    // finger, which would otherwise be a tap.
    let mut caught = false;
    for attempt in 0..5u64 {
        let swipe = 1370 + 2 * attempt;
        let landing = swipe + 1;
        ready(&view, cx);
        finger(cx, swipe, TouchPhase::Started, start);
        for step in 1..=4 {
            std::thread::sleep(Duration::from_millis(8));
            ready(&view, cx);
            let y = step as f32 * 2. * row;
            finger(cx, swipe, TouchPhase::Moved, start + point(px(0.), px(y)));
        }
        ready(&view, cx);
        finger(
            cx,
            swipe,
            TouchPhase::Ended,
            start + point(px(0.), px(8. * row)),
        );
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
    assert!(caught, "no swipe coasted in 5 tries");
    assert_eq!(view.read_with(cx, |this, _| this.taps), taps, "a tap");
    view.read_with(cx, |this, _| assert!(this.handoff.selection.is_none()));
}
