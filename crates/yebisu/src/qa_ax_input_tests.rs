//! The receipt rules: which launches take receipts in, what each herdr-uikit
//! event becomes in the observation file and in `qa.state`'s `input`, and
//! that the queue never waits. Events are emitted with the same field types
//! herdr-uikit `f9fcab6:src/platform/qa_input.rs` uses.
use super::*;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use tracing::{Metadata, Subscriber, span};

struct Into(Arc<Receipts>);
impl Subscriber for Into {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }
    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        self.0.capture(event);
    }
    fn enter(&self, _: &span::Id) {}
    fn exit(&self, _: &span::Id) {}
}

/// The receipts and the unknown count that `emit`'s events leave.
fn captured(emit: impl FnOnce()) -> (Vec<Receipt>, u64) {
    let receipts = Arc::new(Receipts::new(BOUND));
    tracing::subscriber::with_default(Into(receipts.clone()), emit);
    (receipts.take(BOUND), receipts.unknown())
}

fn one(emit: impl FnOnce()) -> Receipt {
    let (taken, unknown) = captured(emit);
    assert_eq!((taken.len(), unknown), (1, 0), "{taken:?}");
    taken[0]
}

fn applied(receipt: &Receipt) -> Counters {
    let mut counters = Counters::default();
    counters.apply(receipt);
    counters
}

#[test]
fn receipts_only_in_a_simulator_build_with_a_qa_variable() {
    assert!(!wanted(false, |_| true), "device or host build");
    assert!(!wanted(false, |_| false));
    assert!(!wanted(true, |_| false), "Simulator build, no QA variable");
    assert!(wanted(true, |name| name == "YEBISU_QA_AX"));
    assert!(wanted(true, |name| name == "YEBISU_P12_QA"));
    assert!(!wanted(true, |name| name == "YEBISU_SMOKE_OBSERVE"));
    assert!(!active(), "host tests are not a Simulator build");
    assert_eq!(TARGET, "herdr_uikit::qa_input");
}

#[test]
fn every_touch_stage_and_kind_is_recorded_and_counted() {
    for (stage, field) in [("began", 0usize), ("ended", 1), ("cancelled", 2)] {
        for kind in ["direct", "pointer"] {
            let receipt = one(|| {
                tracing::info!(target: "herdr_uikit::qa_input", receipt = "touch", stage, kind,
                    x = 12.5_f64, y = 40.0_f64, touches = 1_u64);
            });
            assert_eq!(
                record_json(&receipt),
                json!({"kind": "input-receipt", "receipt": "touch", "stage": stage,
                    "touch_kind": kind, "x": 12.5, "y": 40.0, "touches": 1})
            );
            let counters = applied(&receipt);
            let counts = [
                counters.touch_began,
                counters.touch_ended,
                counters.touch_cancelled,
            ];
            let mut expected = [0; 3];
            expected[field] = 1;
            assert_eq!(counts, expected, "{stage} {kind}");
            assert_eq!(counters.last_touch, Some((stage, kind, 12.5, 40.0)));
            assert_eq!((counters.menu_perform, counters.menu_dropped), (0, 0));
            assert_eq!(
                state_json(&counters)["last_touch"],
                json!({"stage": stage, "kind": kind, "x": 12.5, "y": 40.0})
            );
        }
    }
}

#[test]
fn every_menu_perform_outcome_is_recorded_and_counted() {
    let no_sender = one(|| {
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-perform",
            outcome = "no_sender", current = -1_i64);
    });
    assert_eq!(
        record_json(&no_sender),
        json!({"kind": "input-receipt", "receipt": "menu-perform", "outcome": "no_sender", "current": -1})
    );
    let no_tag = one(|| {
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-perform",
            outcome = "no_tag", current = 3_i64);
    });
    assert_eq!(
        record_json(&no_tag),
        json!({"kind": "input-receipt", "receipt": "menu-perform", "outcome": "no_tag", "current": 3})
    );
    let performed = one(|| {
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-perform",
            outcome = "performed", generation = 3_u64, index = 7_u64, current = 3_i64, keyed = false);
    });
    assert_eq!(
        record_json(&performed),
        json!({"kind": "input-receipt", "receipt": "menu-perform", "outcome": "performed",
            "generation": 3, "index": 7, "current": 3, "keyed": false})
    );
    for (receipt, performed_count, dropped) in
        [(no_sender, 0, 1), (no_tag, 0, 1), (performed, 1, 0)]
    {
        let counters = applied(&receipt);
        assert_eq!(
            (
                counters.menu_perform,
                counters.menu_performed,
                counters.menu_dropped,
                counters.menu_delivered
            ),
            (1, performed_count, dropped, 0),
            "{receipt:?}"
        );
        assert_eq!(counters.last_touch, None);
    }
}

#[test]
fn every_menu_deliver_outcome_is_recorded_and_counted() {
    for outcome in [
        "stale_generation",
        "no_callback",
        "callback_running",
        "delivered",
    ] {
        let receipt = one(|| {
            tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-deliver", outcome,
                generation = 2_u64, index = 5_u64, current = 4_i64);
        });
        assert_eq!(
            record_json(&receipt),
            json!({"kind": "input-receipt", "receipt": "menu-deliver", "outcome": outcome,
                "generation": 2, "index": 5, "current": 4})
        );
        let counters = applied(&receipt);
        let delivered = u64::from(outcome == "delivered");
        assert_eq!(
            (
                counters.menu_delivered,
                counters.menu_dropped,
                counters.menu_perform
            ),
            (delivered, 1 - delivered, 0),
            "{outcome}"
        );
    }
}

#[test]
fn menu_global_is_recorded_and_counted() {
    assert_eq!(
        record_json(&Receipt::MenuGlobal),
        json!({"kind": "input-receipt", "receipt": "menu-global"})
    );
    let counters = applied(&Receipt::MenuGlobal);
    assert_eq!((counters.menu_global, counters.menu_dropped), (1, 0));
}

#[test]
fn unknown_values_give_no_receipt_and_are_counted() {
    let (taken, unknown) = captured(|| {
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "touch", stage = "moved-secret",
            kind = "direct", x = 1.0_f64, y = 2.0_f64, touches = 1_u64);
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "touch", stage = "began",
            kind = "stylus-secret", x = 1.0_f64, y = 2.0_f64, touches = 1_u64);
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-perform",
            outcome = "secret-outcome", current = 1_i64);
        // A deliver outcome is not a perform outcome.
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-perform",
            outcome = "delivered", current = 1_i64);
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-deliver",
            outcome = "performed", generation = 1_u64, index = 1_u64, current = 1_i64);
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "secret-receipt");
        // A required field is missing.
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "touch", stage = "began", kind = "direct");
        // An unknown value in a listed field this receipt does not use still rejects it.
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "menu-perform",
            outcome = "no_tag", current = 1_i64, stage = "secret-stage");
    });
    assert!(taken.is_empty(), "{taken:?}");
    assert_eq!(unknown, 8);
}

#[test]
fn no_string_the_sdk_sends_is_copied() {
    let receipt = one(|| {
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "touch", stage = "began",
            kind = "direct", x = 1.0_f64, y = 2.0_f64, touches = 1_u64, typed_key = "secret-key",
            "secret-message");
    });
    let text = record_json(&receipt).to_string();
    assert!(!text.contains("secret"), "{text}");
    let mut counters = applied(&receipt);
    counters.unknown = 1;
    assert!(!state_json(&counters).to_string().contains("secret"));
    // Every kept string is `&'static`, one of the module's own lists: an
    // event's string lives only as long as the event.
    let Receipt::Touch { stage, kind, .. } = receipt else {
        panic!("not a touch: {receipt:?}");
    };
    assert!(STAGES.contains(&stage) && KINDS.contains(&kind));
}

#[test]
fn a_full_queue_drops_and_counts_without_waiting() {
    let receipts = Arc::new(Receipts::new(BOUND));
    let (done, finished) = mpsc::channel();
    let pusher = receipts.clone();
    std::thread::spawn(move || {
        for _ in 0..BOUND + 5 {
            pusher.push(Receipt::MenuGlobal);
        }
        let _ = done.send(());
    });
    assert!(
        finished.recv_timeout(Duration::from_secs(10)).is_ok(),
        "push waited for room in a full queue"
    );
    assert_eq!(receipts.dropped(), 5);
    assert_eq!(receipts.take(BOUND * 2).len(), BOUND);
    assert!(receipts.take(1).is_empty());
}

#[test]
fn one_drain_takes_at_most_its_limit_in_order() {
    let receipts = Receipts::new(BOUND);
    for touches in 0..10 {
        receipts.push(Receipt::Touch {
            stage: "began",
            kind: "direct",
            x: 0.0,
            y: 0.0,
            touches,
        });
    }
    let first = receipts.take(4);
    let order: Vec<u64> = first
        .iter()
        .filter_map(|receipt| match receipt {
            Receipt::Touch { touches, .. } => Some(*touches),
            _ => None,
        })
        .collect();
    assert_eq!(order, [0, 1, 2, 3]);
    assert_eq!(receipts.take(BOUND).len(), 6);
}

#[test]
fn settle_reports_a_change_only_when_a_counter_moved() {
    let mut counters = Counters::default();
    assert!(!settle(&mut counters, &[], 0, 0), "nothing moved");
    assert!(settle(&mut counters, &[Receipt::MenuGlobal], 0, 0));
    assert!(!settle(&mut counters, &[], 0, 0));
    assert!(settle(&mut counters, &[], 2, 0), "drops are never silent");
    assert!(
        settle(&mut counters, &[], 2, 1),
        "unknown events are counted"
    );
    assert_eq!(
        (counters.menu_global, counters.dropped, counters.unknown),
        (1, 2, 1)
    );
    let input = state_json(&counters);
    assert_eq!(input["dropped"], 2);
    assert_eq!(input["unknown"], 1);
    assert_eq!(input["last_touch"], Value::Null);
    for key in [
        "touch_began",
        "touch_ended",
        "touch_cancelled",
        "menu_perform",
        "menu_performed",
        "menu_delivered",
        "menu_dropped",
        "menu_global",
    ] {
        assert!(input[key].is_u64(), "{key}");
    }
}

#[test]
fn a_heartbeat_every_heartbeat_ticks() {
    // 1 to 5 s (issue #142 stage 2-5), so a 25 s or 60 s stall leaves a gap.
    const { assert!(HEARTBEAT_TICKS >= 1 && HEARTBEAT_TICKS <= 5) };
    let beating: Vec<u64> = (1..=10).filter(|tick| beats(*tick)).collect();
    assert_eq!(beating, [2, 4, 6, 8, 10]);
}

#[test]
fn every_frame_outcome_is_recorded_and_kept_as_the_last_one() {
    for (outcome, previous) in [
        ("presented", "none"),
        ("zero_size", "presented"),
        ("paths_not_ready", "zero_size"),
        ("no_drawable", "paths_not_ready"),
        ("render_failed", "no_drawable"),
    ] {
        let receipt = one(|| {
            tracing::info!(target: "herdr_uikit::qa_input", receipt = "frame", outcome, previous,
                previous_count = 42_u64);
        });
        assert_eq!(
            record_json(&receipt),
            json!({"kind": "input-receipt", "receipt": "frame", "outcome": outcome,
                "previous": previous, "previous_count": 42})
        );
        let counters = applied(&receipt);
        assert_eq!(counters.last_frame, Some(outcome));
        assert_eq!(state_json(&counters)["frame_last"], json!(outcome));
        // A frame receipt is not a touch or a menu command.
        assert_eq!(
            (
                counters.touch_began,
                counters.menu_perform,
                counters.menu_dropped
            ),
            (0, 0, 0)
        );
    }
    assert_eq!(state_json(&Counters::default())["frame_last"], Value::Null);
}

#[test]
fn a_frame_receipt_with_an_unknown_or_missing_value_is_not_taken() {
    let (taken, unknown) = captured(|| {
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "frame", outcome = "secret-outcome",
            previous = "none", previous_count = 1_u64);
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "frame", outcome = "presented",
            previous = "secret-previous", previous_count = 1_u64);
        // A menu outcome is not a draw's outcome.
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "frame", outcome = "delivered",
            previous = "none", previous_count = 1_u64);
        tracing::info!(target: "herdr_uikit::qa_input", receipt = "frame", outcome = "presented",
            previous = "none");
    });
    assert!(taken.is_empty(), "{taken:?}");
    assert_eq!(unknown, 4);
}
