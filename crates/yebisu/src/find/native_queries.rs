//! Opt-in receiving query probe on captured real native clients. No OS oracle.
use crate::app::Smoke;
use gpui::{
    AnyElement, AnyWindowHandle, AsyncWindowContext, ClipboardItem, Context, WeakEntity, div,
    prelude::*,
};
use herdr_uikit::retirement_probe::{
    NativeClient, NativeCutProbeResult, NativeQuery, NativeQueryError, capture, key,
    next_native_turn,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

thread_local! {
    static CAPTURED: RefCell<Option<NativeClient>> = const { RefCell::new(None) };
}
static STEP: AtomicUsize = AtomicUsize::new(0);
const SENTINEL: &str = "p13-query-sentinel";

fn number(result: Result<NativeQuery<Option<usize>>, NativeQueryError>) -> Value {
    match result {
        Ok(q) => json!({"client":q.client,"value":q.value}),
        Err(e) => json!({"error":format!("{e:?}")}),
    }
}
fn selection(client: &NativeClient) -> Value {
    match client.selected_range_utf16() {
        Ok(q) => json!({"client":q.client,"value":q.value}),
        Err(e) => json!({"error":format!("{e:?}")}),
    }
}
fn setup(client: &NativeClient, range: Range<usize>) -> Value {
    match client.select_utf16(range) {
        Ok(q) => json!({"client":q.client,"value":q.value}),
        Err(e) => json!({"error":format!("{e:?}")}),
    }
}
fn observe(client: &NativeClient, label: &str, from: usize, delta: isize) -> Value {
    let before = selection(client);
    rest(client, label, before, from, delta)
}
// Everything after the FIRST selection observation. No setter/focus/handler
// operation repairs an observation. Values come only from SDK selectors.
fn rest(client: &NativeClient, label: &str, before: Value, from: usize, delta: isize) -> Value {
    let document = number(client.document_length_utf16());
    let position = number(client.position_offset_utf16(from, delta));
    let beyond = number(client.position_offset_utf16(from, delta + 1));
    let after = selection(client);
    json!({"kind":"p13-native-query","label":label,"captured":client.identity(),
        "before":before,"document":document,"position":position,"beyond":beyond,"after":after})
}
// The SDK's stored result and passive trace, unchanged; no text or keys.
fn cut_record(result: &NativeCutProbeResult) -> (Value, Value) {
    let (observation, reason, sent) = match result {
        Ok(o) => (Some(o), None, true),
        Err(f) => (
            f.observation.as_ref(),
            Some(format!("{:?}", f.reason)),
            f.selector_sent,
        ),
    };
    let Some(o) = observation else {
        let first = json!({"error":"NoObservation"});
        return (
            first,
            json!({"ok":false,"reason":reason,"selector_sent":sent}),
        );
    };
    let first = match &o.first_selection {
        Ok(q) => json!({"client":q.client,"value":q.value}),
        Err(e) => json!({"error":format!("{e:?}")}),
    };
    let events: Vec<Value> = o
        .trace
        .events
        .iter()
        .flatten()
        .map(|e| {
            json!({"sequence":e.sequence,"phase":format!("{:?}",e.phase),
                "refusal":e.refusal.map(|r|format!("{r:?}")),"range":e.range.map(|(a,b)|[a,b])})
        })
        .collect();
    let trace = json!({"ok":result.is_ok(),"reason":reason,"selector_sent":sent,"run":o.run,
        "client":o.client,"selector_returned":o.selector_returned,"refused":o.trace.refused,
        "len":o.trace.len,"counters":o.trace.counters,"events":events});
    (first, trace)
}
// Q03: one actual production `cut:` joined to this same retained receiver's
// FIRST selection query (SDK helper), then the remaining queries, then state.
fn cut_q03(client: &NativeClient, done: impl FnOnce(Value) + 'static) {
    let captured = client.clone();
    let finish = move |cut: NativeCutProbeResult| {
        let (first, trace) = cut_record(&cut);
        let mut q = rest(&captured, "Q03", first, 5, 6);
        q["cut"] = trace;
        done(q);
    };
    finish(client.cut_and_observe_selection());
}

impl Smoke {
    pub(super) fn p13_query_control(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("p13-query-next")
            .px_4()
            .py_2()
            .child("P13 Next")
            .on_click(cx.listener(|this, _, window, cx| {
                // The peer's idle bound counts from this first fixture tap.
                crate::find::peer::arm_fixture();
                // One tap runs every step; later taps would move UIKit focus
                // off the field and detach its native session mid-sequence.
                if STEP.fetch_add(1, Ordering::Relaxed) != 0 {
                    return;
                }
                // Records read the pasteboard; content from another install
                // makes iOS ask for paste permission and blocks the probe.
                cx.write_to_clipboard(ClipboardItem::new_string(SENTINEL.into()));
                this.open_find(window, cx);
                let weak = cx.entity().downgrade();
                query_step(0, window.window_handle(), window.to_async(cx), weak);
            }))
            .into_any_element()
    }
}
// Each step runs on its own native turn and schedules the next one itself.
fn query_step(
    step: usize,
    handle: AnyWindowHandle,
    mut context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
) {
    next_native_turn(move || {
        let client = if step == 0 {
            let current = capture(handle);
            CAPTURED.with_borrow_mut(|slot| *slot = current.clone());
            current
        } else {
            CAPTURED.with_borrow(Clone::clone)
        };
        let Some(client) = client else {
            return;
        };
        let record = match step {
            0 => {
                client.insert("abcdef");
                let set = setup(&client, 3..3);
                let mut q = observe(&client, "Q01", 3, 3);
                q["setup"] = json!([set]);
                q["one"] = number(client.position_offset_utf16(3, 1));
                q["overflow"] = number(client.position_offset_utf16(usize::MAX, 0));
                q
            }
            1 => {
                let all = setup(&client, 0..6);
                client.insert("pre::post");
                let caret = setup(&client, 4..4);
                client.mark("変😀中");
                let selected = setup(&client, 5..7);
                let mut q = observe(&client, "Q02", 5, 8);
                q["setup"] = json!([all, caret, selected]);
                q
            }
            // Q03 was captured with the Cut; this phase only injects Escape.
            2 => {
                // Production composing Escape, never native unmark/commit. This
                // is an injected native-origin event, not physical-key evidence.
                let handled = key(handle, "escape", true);
                let _ = context.update(|_, cx| {
                    let _ = weak.update(cx, |this, _| {
                        this.observation
                            .record(json!({"kind":"p13-query-escape","handled":handled}));
                    });
                });
                query_step(3, handle, context, weak);
                return;
            }
            3 => observe(&client, "Q04", 4, 5),
            4 => {
                let _ = context.update(|window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.p13_native_barrier("query-before-close");
                        this.close_find(window, cx);
                        cx.notify();
                    });
                });
                super::native_probe::after_input_settles(
                    context,
                    weak,
                    move |mut context, weak, settled, turns| {
                        let Some(current) = capture(handle) else {
                            return;
                        };
                        client.mark("Q05-fresh");
                        client.insert("Q05-fresh");
                        client.unmark();
                        client.delete();
                        client.edit_actions();
                        let _ = context.update(|_, cx| {
                            let _ = weak.update(cx, |this, _| {
                                this.p13_native_barrier("query-old-after-close")
                            });
                        });
                        current.mark("abcdef");
                        let ready = current.text_readiness();
                        let mut q = observe(&current, "Q05-current", 0, 6);
                        // Query the retained old endpoint, never recapture it.
                        q["old"] = observe(&client, "Q05-old", 0, 0);
                        q["ready"] = json!([ready.0, ready.1, ready.2]);
                        q["settled"] = json!(settled);
                        q["settle_turns"] = json!(turns);
                        current.insert("Q05-fresh");
                        let _ = context.update(|_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.p13_native_barrier("query-fresh-after-close");
                                q["paused"] = json!(this.find_handoff.paused);
                                q["reason"] = json!(this.find_handoff.reason);
                                this.observation.record(q);
                                cx.notify();
                            });
                        });
                        query_step(5, handle, context, weak);
                    },
                );
                return;
            }
            5 => json!({"kind":"p13-query-done","captured":client.identity()}),
            _ => return,
        };
        publish(&mut context, &weak, record);
        if step == 1 {
            // Same turn, no wait: Q02 state is already stored above.
            cut_q03(&client, move |q| {
                publish(&mut context, &weak, q);
                query_step(2, handle, context, weak);
            });
        } else if step < 5 {
            query_step(step + 1, handle, context, weak);
        }
    });
}
// State is read only AFTER the first native selection observation.
fn publish(context: &mut AsyncWindowContext, weak: &WeakEntity<Smoke>, mut record: Value) {
    let _ = context.update(|_, cx| {
        let _ = weak.update(cx, |this, cx| {
            if let Some(bar) = &this.find {
                let field = bar.field.read(cx);
                record["text"] = json!(field.input.text);
                record["marked"] = json!(field.input.marked);
                record["model_selection"] = json!(field.input.selected);
            }
            record["clipboard"] = json!(cx.read_from_clipboard().and_then(|i| i.text()));
            this.observation.record(record);
            cx.notify();
        });
    });
}
