//! Opt-in actual UIKit selectors; this does not emulate OS queued input.
use crate::app::Smoke;
use gpui::{AnyWindowHandle, AsyncWindowContext, Context, EntityInputHandler, WeakEntity, Window};
use herdr_uikit::retirement_probe::{capture, next_native_turn};
use std::time::{Duration, Instant};

/// Longest wait for the post-acknowledgement frame and the SDK's responder
/// update after it. Past it the probe reads readiness anyway; the oracle
/// rejects `settled: false` and any readiness that is not all true.
const SETTLE: Duration = Duration::from_secs(2);
/// Longest wait for held terminal input to leave before an after-close
/// barrier. Past it the barrier goes out anyway, recorded `flushed: false`,
/// which the oracles reject by name.
const FLUSH: Duration = Duration::from_secs(5);

/// What the probe waited for before its readiness read.
#[derive(Clone, Copy)]
pub(super) struct Settle {
    /// GPUI drew a frame after the accepted retirement acknowledgement.
    pub drawn: bool,
    /// After that frame, the current native client read ready on all three
    /// values at least once.
    pub responder: bool,
    pub turns: u32,
}

/// Runs `then` on the first native turn after GPUI drew a frame following the
/// accepted retirement acknowledgement (`Smoke::find_input_settled`) and the
/// current native client then read ready, or once `SETTLE` passed.
///
/// GPUI tells the SDK about text input focus only inside `Window::draw`
/// (gpui-pre 0.3.12 window.rs:3733-3740). The SDK applies a focus change to
/// UIKit's first responder two asynchronous hops later: `set_text_input_focused`
/// defers `update_first_responder` to a later main-queue turn (herdr-uikit
/// 33e288d window/lifecycle.rs:370-383, 424-475), whose `activate` schedules a
/// run-loop flush (text_input/sync.rs:25-37, run_loop.rs:18-26) that calls
/// `becomeFirstResponder` (text_input/sync.rs:92-122).
/// `text_readiness` only reads; it never requests focus or retries acquisition
/// (text_input/probe.rs:51-60). Probe scheduling only: each turn reads state
/// and never touches retirement or input.
pub(super) fn after_input_settles(
    handle: AnyWindowHandle,
    context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
    then: impl FnOnce(AsyncWindowContext, WeakEntity<Smoke>, Settle) + 'static,
) {
    settle_turn(handle, context, weak, Instant::now() + SETTLE, 1, then);
}
fn settle_turn(
    handle: AnyWindowHandle,
    mut context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
    deadline: Instant,
    turns: u32,
    then: impl FnOnce(AsyncWindowContext, WeakEntity<Smoke>, Settle) + 'static,
) {
    next_native_turn(move || {
        let drawn = context
            .update(|_, cx| {
                weak.upgrade()
                    .is_some_and(|view| view.read(cx).find_input_settled())
            })
            .unwrap_or(false);
        let responder = drawn
            && capture(handle).is_some_and(|client| client.text_readiness() == (true, true, true));
        if responder || Instant::now() >= deadline {
            let settle = Settle {
                drawn,
                responder,
                turns,
            };
            then(context, weak, settle);
        } else {
            settle_turn(
                handle,
                context,
                weak,
                deadline,
                turns.saturating_add(1),
                then,
            );
        }
    });
}

/// Sends after-close barrier `label` on the first native turn where the
/// terminal input holds nothing and its last sync found the surface at the
/// requested size (`TerminalInput::flushed`), or once `FLUSH` passed, then
/// runs `then` on that same turn.
///
/// Close can resize the terminal. Until the surface matches the requested
/// size, production holds generated input (terminal_input.rs:104-110, tested
/// contract terminal_input_tests.rs:181-212) and flushes it when the matching
/// surface arrives (terminal_view.rs:201-207). A barrier request goes straight
/// to the ordered writer, so one sent earlier overtakes the held input and
/// breaks the oracle's FIFO phases although the input itself is correct.
/// Probe scheduling only: each turn reads state; flushing stays with
/// production. The gate read and the barrier share one update, so nothing
/// runs between them. The `p13-barrier` record carries `flushed`; the oracles
/// reject `false` by name and still check the wire order on their own.
pub(super) fn barrier_after_flush(
    context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
    label: &'static str,
    then: impl FnOnce(AsyncWindowContext, WeakEntity<Smoke>) + 'static,
) {
    flush_turn(context, weak, label, Instant::now() + FLUSH, 1, then);
}
fn flush_turn(
    mut context: AsyncWindowContext,
    weak: WeakEntity<Smoke>,
    label: &'static str,
    deadline: Instant,
    turns: u32,
    then: impl FnOnce(AsyncWindowContext, WeakEntity<Smoke>) + 'static,
) {
    next_native_turn(move || {
        let sent = context
            .update(|_, cx| {
                weak.update(cx, |this, _| {
                    let flushed = this.terminal.input.flushed();
                    if !flushed && Instant::now() < deadline {
                        return false;
                    }
                    this.observation.record(serde_json::json!({
                        "kind":"p13-barrier","label":label,"flushed":flushed,
                        "ready":this.terminal.input.ready,"turns":turns
                    }));
                    this.p13_native_barrier(label);
                    true
                })
                .unwrap_or(true)
            })
            .unwrap_or(true);
        if sent {
            then(context, weak);
        } else {
            flush_turn(
                context,
                weak,
                label,
                deadline,
                turns.saturating_add(1),
                then,
            );
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
            after_input_settles(handle, context, weak, move |context, weak, settle| {
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
                barrier_after_flush(context, weak, "old-after-close", move |context, weak| {
                    // No GPUI KeyDown or terminal tap precedes these selectors.
                    new.insert("P13-fresh");
                    new.mark("P13-fresh");
                    new.insert("P13-fresh");
                    barrier_after_flush(
                        context,
                        weak,
                        "fresh-after-close",
                        move |mut context, weak| {
                            let _ = context.update(|_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.observation.record(serde_json::json!({
                                    "kind":"p13-native-close-probe","old":ids.0,"new":ids.1,
                                    "ready":[ready.0,ready.1,ready.2],"paused":this.find_handoff.paused,
                                    "settled":settle.drawn,"responder":settle.responder,"settle_turns":settle.turns,
                                    "receipt":this.find_handoff.receipt.as_ref().map(|r|format!("{:?}",r.status())),
                                    "reason":this.find_handoff.reason,
                                    "physical_ime":"unverified","os_retarget_loss":"unverified"
                                }));
                                cx.notify();
                            });
                        });
                        },
                    );
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
