//! R5 to R10 without GPUI: routing in `ModViewState`, the bounded queue and
//! the wire `Action` an event becomes.
use super::events::EVENT_CAPACITY;
use super::*;

fn state(fixture: Fixture) -> ModViewState {
    let mut state = ModViewState::default();
    state.install(fixture.adopt());
    assert!(state.scene.is_some(), "{:?}", state.error);
    state
}

#[test]
fn r5_a_tap_on_a_callback_node_queues_its_target_in_order() {
    let mut state = state(Fixture::Counter);
    assert!(state.hit(Hit::Tap(4)));
    assert!(state.hit(Hit::Tap(3)));
    let events = state.events.drain();
    let targets: Vec<(usize, u64)> = events.iter().map(|e| (e.target, e.callback)).collect();
    assert_eq!(targets, [(4, 8), (3, 7)]);
    assert!(events.iter().all(|e| e.kind == EventKind::Click
        && e.generation == 1
        && e.pub_rev == 1
        && e.key.is_none()));
    assert_eq!(state.events.len(), 0);
}

#[test]
fn r6_a_tap_on_a_node_without_a_callback_queues_nothing() {
    let mut state = state(Fixture::Status);
    for index in 0..4 {
        assert!(!state.hit(Hit::Tap(index)));
    }
    assert!(!state.hit(Hit::Tap(99)));
    assert_eq!(state.events.len(), 0);
}

#[test]
fn r7_keys_go_to_the_last_tapped_key_node_only() {
    let mut state = state(Fixture::Form);
    assert!(!state.hit(Hit::Key("a".into())));
    assert!(state.hit(Hit::Tap(2)));
    assert_eq!(state.focused, Some(2));
    assert!(state.hit(Hit::Key("a".into())));
    // The Send button takes taps but not keys: focus stays on the field.
    assert!(state.hit(Hit::Tap(4)));
    assert_eq!(state.focused, Some(2));
    let events = state.events.drain();
    let key = &events[1];
    assert_eq!(
        (key.target, key.callback, key.kind, key.key.as_deref()),
        (2, 12, EventKind::Key, Some("a"))
    );
    assert_eq!((events[2].target, events[2].callback), (4, 13));
}

#[test]
fn r7_a_new_generation_drops_the_focused_node() {
    let mut state = state(Fixture::Form);
    assert!(state.hit(Hit::Tap(2)));
    let mut next = Fixture::Form
        .adopt()
        .unwrap_or_else(|e| panic!("form: {e}"));
    next.generation = 2;
    state.install(Ok(next));
    assert_eq!(state.focused, None);
    assert!(!state.hit(Hit::Key("a".into())));
}

#[test]
fn r8_a_faded_region_queues_nothing() {
    let mut state = state(Fixture::Form);
    assert!(state.hit(Hit::Tap(2)));
    state.faded = true;
    assert!(!state.hit(Hit::Tap(2)));
    assert!(!state.hit(Hit::Key("a".into())));
    assert_eq!(state.events.len(), 1);
}

#[test]
fn r9_a_full_queue_drops_and_counts_new_events() {
    let mut state = state(Fixture::Counter);
    for _ in 0..EVENT_CAPACITY {
        assert!(state.hit(Hit::Tap(3)));
    }
    assert!(!state.hit(Hit::Tap(4)));
    assert!(!state.hit(Hit::Tap(4)));
    assert_eq!(state.events.len(), EVENT_CAPACITY);
    assert_eq!(state.events.dropped(), 2);
    let events = state.events.drain();
    assert!(events.iter().all(|e| e.target == 3));
    assert!(state.hit(Hit::Tap(4)));
}

#[test]
fn r10_an_event_becomes_a_valid_wire_action_with_its_target() -> Result<()> {
    let mut state = state(Fixture::Form);
    state.hit(Hit::Tap(2));
    state.hit(Hit::Key("a".into()));
    let events = state.events.drain();
    let click = events[0].to_action(5, "event-5".into(), "lease-1".into())?;
    assert_eq!(
        (
            click.target,
            click.kind,
            click.generation.0,
            click.pub_rev.0
        ),
        (2, yebisu_mod_wire::ActionKind::Click, 1, 1)
    );
    assert_eq!(click.op_seq.0, 5);
    let key = events[1].to_action(6, "event-6".into(), "lease-1".into())?;
    assert_eq!(key.kind, yebisu_mod_wire::ActionKind::Key);
    assert_eq!(key.data.key.as_deref(), Some("a"));
    // The digest covers the body: a changed target no longer validates.
    let mut forged = key.clone();
    forged.target = 4;
    assert_eq!(
        forged.validate(),
        Err(yebisu_mod_wire::Error::DigestMismatch)
    );
    key.validate()?;
    Ok(())
}
