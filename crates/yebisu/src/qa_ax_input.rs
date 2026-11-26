//! herdr-uikit's input receipts, taken in for Simulator QA (issue #142,
//! stage 2).
//!
//! herdr-uikit's Simulator build emits one `tracing` event with the target
//! `herdr_uikit::qa_input` when a touch reaches the drawing view
//! (`receipt = "touch"`), when UIKit performs a menu command
//! (`"menu-perform"`) and when that command is handed to GPUI, or not
//! (`"menu-deliver"`), and when a window's draws change outcome (`"frame"`,
//! issue #197); see herdr-uikit `docs/qa-input-receipts.md`. With
//! `YEBISU_QA_AX` or `YEBISU_P12_QA` set, the diagnostics subscriber passes
//! those events to `capture`, which keeps only values from the closed lists
//! below (no string the SDK sends is copied) and queues them without ever
//! blocking. The terminal poll (`drain`, every 33 ms on GPUI's foreground
//! thread) writes each one to the observation file as an `input-receipt`
//! record and publishes the counters as the `input` field of `qa.state`, so
//! a test can tell a touch that never reached the app from one that arrived
//! and was not acted on.
//!
//! `menu-global` is yebisu's own receipt: a global `NativeCommand` listener
//! that GPUI calls when the command reached GPUI but no window handler on
//! the dispatch path took it. That is either no active window, or the root
//! view's `on_action` (app.rs) was not on the focused dispatch path: GPUI
//! runs global bubble listeners after a window dispatch that nothing stopped
//! (gpui-pre 0.3.12 `src/window.rs:6865-6895`).
//!
//! Compiled only for the Simulator and for host tests: a device build has no
//! receipts and calls none of this.
#![cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "only a Simulator build calls in; host tests cover the pure parts"
    )
)]
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
};
use tracing::field::{Field, Visit};

/// The `tracing` target of every herdr-uikit receipt. The subscriber in
/// diagnostics.rs matches it (`RECEIPTS`); its tests check the two agree.
#[cfg(test)]
pub(crate) const TARGET: &str = "herdr_uikit::qa_input";
/// The P12 fixture's launch variable; it also turns receipts on.
const P12_ENV: &str = "YEBISU_P12_QA";
/// Receipts queued and not yet drained. A full queue drops the newest
/// receipt and counts it in `dropped`; a 33 ms poll drains far fewer.
pub(crate) const BOUND: usize = 1024;
/// Receipts one poll takes from the queue, so one poll stays short.
const DRAIN: usize = 256;
/// One heartbeat every this many 1 s ticks. A P12 launch (about 180 s)
/// writes at most 90; a gap of more than 2.5 intervals (5 s) is reported.
pub(crate) const HEARTBEAT_TICKS: u64 = 2;

const RECEIPTS: [&str; 4] = ["touch", "menu-perform", "menu-deliver", "frame"];
const STAGES: [&str; 3] = ["began", "ended", "cancelled"];
const KINDS: [&str; 2] = ["direct", "pointer"];
const PERFORM: [&str; 3] = ["no_sender", "no_tag", "performed"];
const DELIVER: [&str; 4] = [
    "stale_generation",
    "no_callback",
    "callback_running",
    "delivered",
];
/// A window draw's outcome (herdr-uikit `Renderer::draw`, issue #197).
const FRAME: [&str; 5] = [
    "presented",
    "zero_size",
    "paths_not_ready",
    "no_drawable",
    "render_failed",
];
/// The outcome of the run a `frame` receipt ends: a draw's outcome, or none
/// before a window's first draw.
const PREVIOUS: [&str; 6] = [
    "none",
    "presented",
    "zero_size",
    "paths_not_ready",
    "no_drawable",
    "render_failed",
];

/// The rule: a Simulator build launched with `YEBISU_QA_AX` or
/// `YEBISU_P12_QA`.
pub(crate) fn wanted(simulator: bool, env_set: impl Fn(&str) -> bool) -> bool {
    simulator && (env_set(super::ENV) || env_set(P12_ENV))
}

/// True only when this process takes in receipts (`wanted` for this build).
pub(crate) fn active() -> bool {
    wanted(super::BUILD, |name| std::env::var_os(name).is_some())
}

/// One receipt. Every string is one of this module's constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Receipt {
    Touch {
        stage: &'static str,
        kind: &'static str,
        x: f64,
        y: f64,
        touches: u64,
    },
    MenuPerform {
        outcome: &'static str,
        generation: Option<u64>,
        index: Option<u64>,
        current: i64,
        keyed: Option<bool>,
    },
    MenuDeliver {
        outcome: &'static str,
        generation: u64,
        index: u64,
        current: i64,
    },
    MenuGlobal,
    /// A window's draws came to a different outcome than the draw before.
    Frame {
        outcome: &'static str,
        previous: &'static str,
        previous_count: u64,
    },
}

/// The fields of one event. A string outside its list sets `rejected`.
#[derive(Debug, Default)]
pub(crate) struct Fields {
    receipt: Option<&'static str>,
    stage: Option<&'static str>,
    kind: Option<&'static str>,
    outcome: Option<&'static str>,
    x: Option<f64>,
    y: Option<f64>,
    touches: Option<u64>,
    generation: Option<u64>,
    index: Option<u64>,
    current: Option<i64>,
    keyed: Option<bool>,
    previous: Option<&'static str>,
    previous_count: Option<u64>,
    rejected: bool,
}

fn known(list: &[&'static str], value: &str) -> Option<&'static str> {
    list.iter().copied().find(|item| *item == value)
}

impl Fields {
    fn text(&mut self, found: Option<&'static str>) -> Option<&'static str> {
        self.rejected |= found.is_none();
        found
    }
    fn unsigned(&mut self, name: &str, value: u64) {
        match name {
            "touches" => self.touches = Some(value),
            "generation" => self.generation = Some(value),
            "index" => self.index = Some(value),
            "current" => self.current = i64::try_from(value).ok(),
            "previous_count" => self.previous_count = Some(value),
            _ => {}
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "receipt" => self.receipt = self.text(known(&RECEIPTS, value)),
            "stage" => self.stage = self.text(known(&STAGES, value)),
            "kind" => self.kind = self.text(known(&KINDS, value)),
            "outcome" => {
                let found = known(&PERFORM, value)
                    .or_else(|| known(&DELIVER, value))
                    .or_else(|| known(&FRAME, value));
                self.outcome = self.text(found);
            }
            "previous" => self.previous = self.text(known(&PREVIOUS, value)),
            _ => {}
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.unsigned(field.name(), value);
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "current" {
            self.current = Some(value);
        } else if let Ok(value) = u64::try_from(value) {
            self.unsigned(field.name(), value);
        }
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        match field.name() {
            "x" => self.x = Some(value),
            "y" => self.y = Some(value),
            _ => {}
        }
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "keyed" {
            self.keyed = Some(value);
        }
    }
    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}

/// The receipt an event's fields describe; `None` for an unknown receipt,
/// stage, kind or outcome, or a missing field.
pub(crate) fn parse(fields: &Fields) -> Option<Receipt> {
    if fields.rejected {
        return None;
    }
    match fields.receipt? {
        "touch" => Some(Receipt::Touch {
            stage: fields.stage?,
            kind: fields.kind?,
            x: fields.x?,
            y: fields.y?,
            touches: fields.touches?,
        }),
        "menu-perform" => Some(Receipt::MenuPerform {
            outcome: fields.outcome.filter(|outcome| PERFORM.contains(outcome))?,
            generation: fields.generation,
            index: fields.index,
            current: fields.current?,
            keyed: fields.keyed,
        }),
        "menu-deliver" => Some(Receipt::MenuDeliver {
            outcome: fields.outcome.filter(|outcome| DELIVER.contains(outcome))?,
            generation: fields.generation?,
            index: fields.index?,
            current: fields.current?,
        }),
        "frame" => Some(Receipt::Frame {
            outcome: fields.outcome.filter(|outcome| FRAME.contains(outcome))?,
            previous: fields.previous?,
            previous_count: fields.previous_count?,
        }),
        _ => None,
    }
}

/// The bounded queue between the subscriber (any thread) and the poll.
pub(crate) struct Receipts {
    sender: SyncSender<Receipt>,
    receiver: Mutex<Receiver<Receipt>>,
    /// Receipts a full queue dropped.
    dropped: AtomicU64,
    /// Events on the target that named no known receipt.
    unknown: AtomicU64,
}

impl Receipts {
    pub(crate) fn new(bound: usize) -> Self {
        let (sender, receiver) = sync_channel(bound);
        Self {
            sender,
            receiver: Mutex::new(receiver),
            dropped: AtomicU64::new(0),
            unknown: AtomicU64::new(0),
        }
    }
    /// Queues a receipt; a full queue drops it and counts the drop. Never
    /// waits (the subscriber runs on UIKit's main thread).
    pub(crate) fn push(&self, receipt: Receipt) {
        if self.sender.try_send(receipt).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    /// Parses one event and queues it, or counts it as unknown.
    pub(crate) fn capture(&self, event: &tracing::Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        match parse(&fields) {
            Some(receipt) => self.push(receipt),
            None => {
                self.unknown.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    /// Up to `limit` queued receipts, oldest first. Empty when another
    /// thread holds the receiver: the poll never waits for it.
    pub(crate) fn take(&self, limit: usize) -> Vec<Receipt> {
        let Ok(receiver) = self.receiver.try_lock() else {
            return Vec::new();
        };
        std::iter::from_fn(|| receiver.try_recv().ok())
            .take(limit)
            .collect()
    }
    pub(crate) fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
    pub(crate) fn unknown(&self) -> u64 {
        self.unknown.load(Ordering::Relaxed)
    }
}

static QUEUE: OnceLock<Receipts> = OnceLock::new();

fn queue() -> &'static Receipts {
    QUEUE.get_or_init(|| Receipts::new(BOUND))
}

/// Queues a receipt for the next poll (any thread, never waits).
pub(crate) fn push(receipt: Receipt) {
    queue().push(receipt);
}

/// The diagnostics subscriber's entry for an event on `TARGET`.
pub(crate) fn capture(event: &tracing::Event<'_>) {
    queue().capture(event);
}

/// The `input` counters of `qa.state`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Counters {
    pub(crate) touch_began: u64,
    pub(crate) touch_ended: u64,
    pub(crate) touch_cancelled: u64,
    /// Every `menu-perform`, whatever its outcome.
    pub(crate) menu_perform: u64,
    pub(crate) menu_performed: u64,
    pub(crate) menu_delivered: u64,
    /// `menu-perform` other than `performed` plus `menu-deliver` other than
    /// `delivered`.
    pub(crate) menu_dropped: u64,
    pub(crate) menu_global: u64,
    pub(crate) dropped: u64,
    pub(crate) unknown: u64,
    /// The last touch: stage, kind, x and y.
    pub(crate) last_touch: Option<(&'static str, &'static str, f64, f64)>,
    /// The outcome the last `frame` receipt named (issue #197).
    pub(crate) last_frame: Option<&'static str>,
}

impl Counters {
    pub(crate) fn apply(&mut self, receipt: &Receipt) {
        match *receipt {
            Receipt::Touch {
                stage, kind, x, y, ..
            } => {
                match stage {
                    "began" => self.touch_began += 1,
                    "ended" => self.touch_ended += 1,
                    _ => self.touch_cancelled += 1,
                }
                self.last_touch = Some((stage, kind, x, y));
            }
            Receipt::MenuPerform { outcome, .. } => {
                self.menu_perform += 1;
                if outcome == "performed" {
                    self.menu_performed += 1;
                } else {
                    self.menu_dropped += 1;
                }
            }
            Receipt::MenuDeliver { outcome, .. } => {
                if outcome == "delivered" {
                    self.menu_delivered += 1;
                } else {
                    self.menu_dropped += 1;
                }
            }
            Receipt::MenuGlobal => self.menu_global += 1,
            Receipt::Frame { outcome, .. } => self.last_frame = Some(outcome),
        }
    }
}

/// The observation record of a receipt (`Observation::try_record` adds the
/// time). The touch's kind is `touch_kind`: `kind` names the record.
pub(crate) fn record_json(receipt: &Receipt) -> Value {
    match *receipt {
        Receipt::Touch {
            stage,
            kind,
            x,
            y,
            touches,
        } => json!({"kind": "input-receipt", "receipt": "touch", "stage": stage,
            "touch_kind": kind, "x": x, "y": y, "touches": touches}),
        Receipt::MenuPerform {
            outcome,
            generation,
            index,
            current,
            keyed,
        } => {
            let mut value = json!({"kind": "input-receipt", "receipt": "menu-perform",
                "outcome": outcome, "current": current});
            if let Some(generation) = generation {
                value["generation"] = generation.into();
            }
            if let Some(index) = index {
                value["index"] = index.into();
            }
            if let Some(keyed) = keyed {
                value["keyed"] = keyed.into();
            }
            value
        }
        Receipt::MenuDeliver {
            outcome,
            generation,
            index,
            current,
        } => json!({"kind": "input-receipt", "receipt": "menu-deliver", "outcome": outcome,
            "generation": generation, "index": index, "current": current}),
        Receipt::MenuGlobal => json!({"kind": "input-receipt", "receipt": "menu-global"}),
        Receipt::Frame {
            outcome,
            previous,
            previous_count,
        } => json!({"kind": "input-receipt", "receipt": "frame", "outcome": outcome,
            "previous": previous, "previous_count": previous_count}),
    }
}

/// The `input` field of `qa.state`.
pub(crate) fn state_json(counters: &Counters) -> Value {
    let last_touch = counters.last_touch.map_or(
        Value::Null,
        |(stage, kind, x, y)| json!({"stage": stage, "kind": kind, "x": x, "y": y}),
    );
    json!({
        "touch_began": counters.touch_began,
        "touch_ended": counters.touch_ended,
        "touch_cancelled": counters.touch_cancelled,
        "menu_perform": counters.menu_perform,
        "menu_performed": counters.menu_performed,
        "menu_delivered": counters.menu_delivered,
        "menu_dropped": counters.menu_dropped,
        "menu_global": counters.menu_global,
        "dropped": counters.dropped,
        "unknown": counters.unknown,
        "last_touch": last_touch,
        "frame_last": counters.last_frame,
    })
}

/// Applies a drained batch and the queue's drop and unknown totals; true
/// when a counter moved.
pub(crate) fn settle(
    counters: &mut Counters,
    batch: &[Receipt],
    dropped: u64,
    unknown: u64,
) -> bool {
    let before = counters.clone();
    for receipt in batch {
        counters.apply(receipt);
    }
    counters.dropped = dropped;
    counters.unknown = unknown;
    *counters != before
}

/// The counters of this process, and whether `qa.state` carries them yet.
#[derive(Default)]
struct Published {
    counters: Counters,
    reported: bool,
}

thread_local! {
    // One set per process: every window's poll drains the same queue.
    static PUBLISHED: RefCell<Published> = RefCell::new(Published::default());
}

/// Writes the queued receipts to `observation` and publishes the counters
/// (also zero ones, so a test can tell "no touch yet" from "no receipts").
/// Call it from the terminal poll on GPUI's foreground thread.
pub(crate) fn drain(observation: &crate::observation::Observation) {
    if !active() {
        return;
    }
    let receipts = queue();
    let batch = receipts.take(DRAIN);
    for receipt in &batch {
        observation.record(record_json(receipt));
    }
    PUBLISHED.with(|published| {
        let mut published = published.borrow_mut();
        let changed = settle(
            &mut published.counters,
            &batch,
            receipts.dropped(),
            receipts.unknown(),
        );
        if changed || !published.reported {
            let counters = published.counters.clone();
            // False before the first frame built the root publisher: try
            // again on the next poll.
            published.reported = crate::qa_ax::report("input", || state_json(&counters));
            if published.reported {
                crate::qa_ax::request_frame();
            }
        }
    });
}

/// Whether the 1 s tick `ticks` writes a heartbeat.
pub(crate) fn beats(ticks: u64) -> bool {
    ticks.is_multiple_of(HEARTBEAT_TICKS)
}

/// A heartbeat in the observation file every `HEARTBEAT_TICKS` ticks. The
/// tick runs on GPUI's foreground thread, so a stalled main thread leaves a
/// gap. Never waits for room in the writer's queue.
pub(crate) fn heartbeat(observation: &crate::observation::Observation, ticks: u64) {
    if active() && beats(ticks) {
        observation.record_without_waiting(json!({"kind": "heartbeat", "tick": ticks}));
    }
}

/// Registers the global `NativeCommand` listener that records `menu-global`.
/// Call it once, in `run()`: every call adds another listener.
pub(crate) fn register_global(cx: &mut gpui::App) {
    if active() {
        cx.on_action(|_: &crate::native_menus::NativeCommand, _| push(Receipt::MenuGlobal));
    }
}

#[cfg(test)]
#[path = "qa_ax_input_tests.rs"]
mod tests;
