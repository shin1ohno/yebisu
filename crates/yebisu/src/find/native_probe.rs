//! Opt-in actual UIKit selectors; this does not emulate OS queued input.
use crate::app::Smoke;
use gpui::{Context, EntityInputHandler, Window};
use herdr_uikit::retirement_probe::{capture, next_native_turn};

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
            next_native_turn(move || {
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
                            "receipt":this.find_handoff.receipt.as_ref().map(|r|format!("{:?}",r.status())),
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
