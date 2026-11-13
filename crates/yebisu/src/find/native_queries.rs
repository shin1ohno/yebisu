//! Opt-in receiving query probe on captured real native clients. No OS oracle.
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window, div, prelude::*};
use herdr_uikit::retirement_probe::{
    NativeClient, NativeQuery, NativeQueryError, capture, key, next_native_turn,
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
    // Selection MUST be first, especially after Cut. No setter/focus/handler
    // operation repairs an observation. Values come only from SDK selectors.
    let before = selection(client);
    let document = number(client.document_length_utf16());
    let position = number(client.position_offset_utf16(from, delta));
    let beyond = number(client.position_offset_utf16(from, delta + 1));
    let after = selection(client);
    json!({"kind":"p13-native-query","label":label,"captured":client.identity(),
        "before":before,"document":document,"position":position,"beyond":beyond,"after":after})
}

impl Smoke {
    pub(super) fn p13_query_control(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("p13-query-next")
            .px_4()
            .py_2()
            .child("P13 Next")
            .on_click(cx.listener(|this, _, window, cx| {
                let step = STEP.fetch_add(1, Ordering::Relaxed);
                if step == 0 {
                    this.open_find(window, cx);
                }
                this.schedule_p13_query(step, window, cx);
            }))
            .into_any_element()
    }
    fn schedule_p13_query(&mut self, step: usize, window: &Window, cx: &Context<Self>) {
        let handle = window.window_handle();
        let mut context = window.to_async(cx);
        let weak = cx.entity().downgrade();
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
            let mut record = match step {
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
                2 => observe(&client, "Q03", 5, 6),
                3 => observe(&client, "Q04", 4, 5),
                4 => {
                    let _ = context.update(|window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.p13_native_barrier("query-before-close");
                            this.close_find(window, cx);
                            cx.notify();
                        });
                    });
                    next_native_turn(move || {
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
                        current.insert("Q05-fresh");
                        let _ = context.update(|_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.p13_native_barrier("query-fresh-after-close");
                                q["paused"] = json!(this.find_handoff.paused);
                                this.observation.record(q);
                                cx.notify();
                            });
                        });
                    });
                    return;
                }
                5 => json!({"kind":"p13-query-done","captured":client.identity()}),
                _ => return,
            };
            // State is read only AFTER the first native selection observation.
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
            if step == 1 {
                // Same existing actual UIKit Cut selector and native-origin guard.
                let scheduled = super::qa::schedule_edit(true);
                let _ = context.update(|_, cx| {
                    let _ = weak.update(cx, |this, _| {
                        this.observation.record(
                            json!({"kind":"p13-query-cut-scheduled","scheduled":scheduled}),
                        );
                    });
                });
            }
            if step == 2 {
                // Production composing Escape, never native unmark/commit. This
                // is an injected native-origin event, not physical-key evidence.
                let handled = key(handle, "escape", true);
                let _ = context.update(|_, cx| {
                    let _ = weak.update(cx, |this, _| {
                        this.observation
                            .record(json!({"kind":"p13-query-escape","handled":handled}));
                    });
                });
            }
        });
    }
}
