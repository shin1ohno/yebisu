//! Opt-in actual UIKit selectors; this does not emulate OS queued input.
use crate::app::Smoke;
use gpui::{AsyncWindowContext, Context, EntityInputHandler, WeakEntity, Window};
use herdr_uikit::retirement_probe::{capture, next_native_turn};
use std::time::{Duration, Instant};

/// Longest wait for the post-acknowledgement frame. Past it the probe reads
/// readiness anyway and records `settled: false`, which the oracle rejects.
const SETTLE: Duration = Duration::from_secs(2);

/// Runs `then` on the first native turn after GPUI drew a frame following the
/// accepted retirement acknowledgement (`Smoke::find_input_settled`), or once
/// `SETTLE` passed. GPUI tells the SDK about text input focus only inside
/// `Window::draw` (gpui-pre 0.3.12 window.rs:3733-3740), so readiness read in
/// `then` is the focus that draw re-evaluated. Probe scheduling only: each
/// turn reads state and never touches retirement or input.
pub(super) fn after_input_settles(
    context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
    then: impl FnOnce(AsyncWindowContext, WeakEntity<Smoke>, bool, u32) + 'static,
) {
    settle_turn(context, weak, Instant::now() + SETTLE, 1, then);
}
fn settle_turn(
    mut context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
    deadline: Instant,
    turns: u32,
    then: impl FnOnce(AsyncWindowContext, WeakEntity<Smoke>, bool, u32) + 'static,
) {
    next_native_turn(move || {
        let settled = context
            .update(|_, cx| {
                weak.upgrade()
                    .is_some_and(|view| view.read(cx).find_input_settled())
            })
            .unwrap_or(false);
        if settled || Instant::now() >= deadline {
            then(context, weak, settled, turns);
        } else {
            settle_turn(context, weak, deadline, turns.saturating_add(1), then);
        }
    });
}

impl Smoke {
    pub(crate) fn p13_native_close_probe(&mut self, window: &Window, cx: &Context<Self>) {
        let handle = window.window_handle();
        let mut context = window.to_async(cx);
        let weak = cx.entity().downgrade();
        next_native_turn(move || {
            let Some(old) = capture(handle) else {
                return;
            };
            let _ = context.update(|window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if let Some(bar) = &this.find {
                        bar.field.update(cx, |field, cx| {
                            let len = field.input.text.encode_utf16().count();
                            field.set_selected_text_range(0..len, window, cx);
                        });
                    }
                });
            });
            // Same actual text before retirement and on the replacement client.
            old.mark("P13-fresh");
            old.insert("P13-fresh");
            let _ = context.update(|window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.p13_native_barrier("old-before-close");
                    this.close_find(window, cx);
                    this.observation.record(serde_json::json!({"kind":"p13-close","focused":this.terminal_focus.is_focused(window),"active":this.find.is_some()}));
                    cx.notify();
                });
            });
            // Probe scheduling only: never retries production retirement or input.
            after_input_settles(context, weak, move |mut context, weak, settled, turns| {
                let Some(new) = capture(handle) else {
                    return;
                };
                let ready = new.text_readiness();
                let ids = (old.identity(), new.identity());
                old.mark("P13-fresh");
                old.insert("P13-fresh");
                old.unmark();
                old.delete();
                old.edit_actions();
                let _ = context.update(|_, cx| {
                    let _ = weak.update(cx, |this, _| {
                        this.p13_native_barrier("old-after-close");
                    });
                });
                // No GPUI KeyDown or terminal tap precedes these selectors.
                new.insert("P13-fresh");
                new.mark("P13-fresh");
                new.insert("P13-fresh");
                let _ = context.update(|_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.p13_native_barrier("fresh-after-close");
                        this.observation.record(serde_json::json!({
                            "kind":"p13-native-close-probe","old":ids.0,"new":ids.1,
                            "ready":[ready.0,ready.1,ready.2],"paused":this.find_handoff.paused,
                            "settled":settled,"settle_turns":turns,
                            "receipt":this.find_handoff.receipt.as_ref().map(|r|format!("{:?}",r.status())),
                            "reason":this.find_handoff.reason,
                            "physical_ime":"unverified","os_retarget_loss":"unverified"
                        }));
                        cx.notify();
                    });
                });
            });
        });
    }
    pub(super) fn p13_native_barrier(&self, label: &str) {
        if let (Some(handle), Some(snapshot)) =
            (&self.terminal.link.handle, &self.terminal.live.snapshot)
        {
            let _ = handle.request(
                &snapshot.boot_id,
                sheprd_client::Method::PaneScroll,
                serde_json::json!({"pane_id":"w1:p1","offset_from_bottom":0,"p13_barrier":label}),
            );
        }
    }
}
