//! The layer's publishing rules, and press counting through GPUI's real
//! mouse dispatch: a press counts only when GPUI's hit test reaches the
//! marked control, so a press an occluding layer takes is not counted.
//! (These run the layer directly; on the host `qa_ax::mark` stays off.)
use super::*;
use gpui::{
    Context, Modifiers, MouseButton, TestAppContext, VisualTestContext, Window, div, point, px,
    size,
};
use serde_json::json;

const ID: &str = "qa.test.target";

fn control(x: f32) -> Control {
    Control {
        id: ID,
        label: "Target",
        frame: [x, 24.0, 44.0, 36.0],
    }
}

#[test]
fn qa_state_seq_moves_only_with_content() {
    let Some(first) = next(None, vec![control(10.0)], State::default()) else {
        panic!("first publish missing");
    };
    assert_eq!(first.1.seq, 1);
    assert!(next(Some(&first), vec![control(10.0)], State::default()).is_none());
    // A moved frame republishes the layout without a new state value.
    let Some(moved) = next(Some(&first), vec![control(12.0)], State::default()) else {
        panic!("moved frame not republished");
    };
    assert_eq!(moved.1.seq, 1);
    let mut pressed = State::default();
    pressed.presses.insert(ID, 1);
    let Some(pressed) = next(Some(&moved), vec![control(12.0)], pressed) else {
        panic!("press not republished");
    };
    assert_eq!(pressed.1.seq, 2);
}

#[test]
fn qa_state_json_is_flat_and_keeps_its_own_fields() {
    let mut state = State {
        seq: 7,
        launch: 42,
        duplicates: vec![ID],
        ..State::default()
    };
    state.presses.insert(ID, 3);
    state.fields.insert("screen", json!("p0"));
    state.fields.insert("menu", Value::Null);
    let value: Value = match serde_json::from_str(&state_json(&state)) {
        Ok(value) => value,
        Err(error) => panic!("state is not JSON: {error}"),
    };
    assert_eq!(
        value,
        json!({"seq": 7, "launch": 42, "presses": {ID: 3}, "duplicates": [ID],
               "screen": "p0", "menu": null})
    );
    assert!(!state_json(&state).contains('\n'), "one line");
}

#[test]
fn report_refuses_the_layers_own_fields() {
    publishing_thread();
    for key in RESERVED {
        assert!(!report(key, json!(1)), "{key}");
    }
    assert!(report("key_ready", json!(true)));
}

/// Review F4: the registry is per thread, so a field reported from a thread
/// that never built the root publisher (a background task) must say it was
/// not taken, and must not appear in `qa.state`.
#[test]
fn report_from_a_thread_without_the_publisher_is_refused() {
    let elsewhere = std::thread::spawn(|| report("key_ready", json!(true)));
    assert!(matches!(elsewhere.join(), Ok(false)));
    assert!(
        !report("key_ready", json!(true)),
        "no publisher on this thread yet"
    );
    publishing_thread();
    assert!(report("key_ready", json!(true)));
}

#[test]
fn collapse_keeps_the_first_mark_of_an_identifier() {
    let other = Control {
        id: "qa.test.other",
        label: "Other",
        frame: [0.0; 4],
    };
    let (controls, duplicates) = collapse(vec![control(1.0), other.clone(), control(2.0)]);
    assert_eq!(controls, vec![other, control(1.0)], "sorted by identifier");
    assert_eq!(duplicates, vec![ID]);
}

/// A control at (100, 100, 80, 40) with the mark, an optional occluding
/// layer over the whole window that takes one press and goes away (like
/// the ... menu's backdrop), and the root publisher.
struct Probe {
    clicks: u64,
    covered: bool,
    twice: bool,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("target")
                    .absolute()
                    .left(px(100.))
                    .top(px(100.))
                    .w(px(80.))
                    .h(px(40.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clicks += 1;
                        cx.notify();
                    }))
                    .child(marker(ID, "Target"))
                    .children(self.twice.then(|| marker(ID, "Target again"))),
            )
            .children(self.covered.then(|| {
                div()
                    .id("cover")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.covered = false;
                            cx.notify();
                        }),
                    )
            }))
            .child(publisher(move |cx| {
                let probe = view.read(cx);
                vec![
                    ("clicks", json!(probe.clicks)),
                    ("covered", json!(probe.covered)),
                ]
            }))
    }
}

fn state(cx: &mut VisualTestContext) -> (Vec<Control>, State) {
    cx.run_until_parked();
    match published() {
        Some(published) => published,
        None => panic!("nothing published"),
    }
}

fn press(cx: &mut VisualTestContext) {
    cx.simulate_click(point(px(140.), px(120.)), Modifiers::none());
}

#[gpui::test]
fn presses_count_only_when_gpui_hits_the_control(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        clicks: 0,
        covered: false,
        twice: false,
    });
    cx.simulate_resize(size(px(600.), px(400.)));
    let (controls, first) = state(cx);
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].frame, [100.0, 100.0, 80.0, 40.0]);
    assert!(first.presses.is_empty());
    assert_eq!(first.fields["clicks"], 0);
    assert_eq!(first.launch, std::process::id());

    press(cx);
    let (_, pressed) = state(cx);
    assert_eq!(pressed.presses.get(ID), Some(&1));
    assert_eq!(pressed.fields["clicks"], 1);
    assert!(pressed.seq > first.seq);

    // Covered: the occluding layer takes the press, the control does not.
    view.update(cx, |this, cx| {
        this.covered = true;
        cx.notify();
    });
    let (_, covered) = state(cx);
    assert_eq!(covered.fields["covered"], true);
    press(cx);
    let (_, taken) = state(cx);
    assert_eq!(
        taken.presses.get(ID),
        Some(&1),
        "a covered press is not counted"
    );
    assert_eq!(taken.fields["clicks"], 1);
    assert_eq!(taken.fields["covered"], false);
    assert!(taken.seq > covered.seq, "the state still moved");

    press(cx);
    let (_, again) = state(cx);
    assert_eq!(again.presses.get(ID), Some(&2));
    assert_eq!(again.fields["clicks"], 2);

    // A press outside the control counts nothing.
    cx.simulate_click(point(px(20.), px(20.)), Modifiers::none());
    let (_, outside) = state(cx);
    assert_eq!(outside.presses.get(ID), Some(&2));
    assert_eq!(outside.seq, again.seq);
}

#[gpui::test]
fn a_press_is_published_even_when_the_control_draws_nothing_new(cx: &mut TestAppContext) {
    struct Inert;
    impl Render for Inert {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .absolute()
                        .left(px(100.))
                        .top(px(100.))
                        .w(px(80.))
                        .h(px(40.))
                        .child(marker(ID, "Inert")),
                )
                .child(publisher(|_| Vec::new()))
        }
    }
    let (_, cx) = cx.add_window_view(|_, _| Inert);
    cx.simulate_resize(size(px(600.), px(400.)));
    let (_, first) = state(cx);
    press(cx);
    let (_, pressed) = state(cx);
    assert_eq!(pressed.presses.get(ID), Some(&1));
    assert_eq!(pressed.seq, first.seq + 1);
}

#[gpui::test]
fn an_identifier_marked_twice_is_reported(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        clicks: 0,
        covered: false,
        twice: true,
    });
    cx.simulate_resize(size(px(600.), px(400.)));
    let (controls, state) = state(cx);
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].label, "Target");
    assert_eq!(state.duplicates, vec![ID]);
}
