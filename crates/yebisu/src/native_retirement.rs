//! Opt-in SDK prototype integration. No retry timer, key replay, or OS guarantee.
use crate::app::Smoke;
use gpui::{AsyncWindowContext, Context, WeakEntity, Window};
use herdr_uikit::{TextInputRetirementError as Error, TextInputRetirementStatus as Status};

#[derive(Clone)]
pub(crate) struct Binding {
    context: AsyncWindowContext,
    entity: WeakEntity<Smoke>,
}

impl Smoke {
    pub(crate) fn bind_text_retirement(&mut self, window: &Window, cx: &Context<Self>) {
        self.retirement.native = Some(Binding {
            context: window.to_async(cx),
            entity: cx.entity().downgrade(),
        });
    }

    pub(crate) fn request_text_retirement(&mut self) {
        if !self.terminal.input.requires_gesture() {
            return;
        }
        let Some(mut binding) = self.retirement.native.clone() else {
            self.retirement.reason = Some("Text input paused · native window unavailable");
            return;
        };
        let handle = binding.context.window_handle();
        let Some(state) = herdr_uikit::text_input_session_state(handle) else {
            self.retirement.reason = Some("Text input paused · native window unavailable");
            return;
        };
        // Do not invalidate the in-flight ticket. Completion checks its captured
        // owner and refuses a changed destination, without retrying it on a timer.
        if state.replacement_pending {
            return;
        }
        let Some(ticket) = self.begin_text_retirement(state.client) else {
            return;
        };
        self.retirement.receipt = None;
        let callback_ticket = ticket.clone();
        let result = herdr_uikit::retire_text_input_session(handle, move |sessions| {
            let _ = binding.context.update(|window, cx| {
                let _ = binding.entity.update(cx, |this, cx| {
                    // The binding is window-scoped, even if the same entity moves.
                    let same_window = this
                        .retirement
                        .native
                        .as_ref()
                        .is_some_and(|b| b.context.window_handle() == window.window_handle());
                    let focus_current =
                        this.terminal.needs_focus || this.terminal_focus.is_focused(window);
                    if !same_window || !focus_current {
                        this.refuse_text_retirement(
                            &callback_ticket,
                            "Text input paused · keyboard owner changed",
                        );
                    }
                    let accepted = same_window
                        && focus_current
                        && this.complete_text_retirement(
                            &callback_ticket,
                            sessions.retired,
                            sessions.replacement,
                        );
                    this.observation.record(serde_json::json!({
                        "kind":"native-retirement-callback", "serial":callback_ticket.serial,
                        "retired":sessions.retired, "replacement":sessions.replacement,
                        "accepted":accepted
                    }));
                    cx.notify();
                });
            });
        });
        match result {
            Ok(receipt) => {
                // Registration preceded the call: its callback may already have
                // run. Receipt assignment must never recreate/clear the ticket.
                self.retirement.receipt = Some(receipt);
            }
            Err(error) => {
                self.observation.record(serde_json::json!({
                    "kind":"native-retirement-refused", "serial":ticket.serial,
                    "reason":format!("{error:?}")
                }));
                self.refuse_text_retirement(
                    &ticket,
                    match error {
                        Error::NativeCallbackInFlight => {
                            "Text input paused · native callback still active"
                        }
                        Error::InProgress => "Text input paused · native replacement pending",
                        Error::IdentityExhausted => "Text input paused · native identity exhausted",
                        Error::NotMainThread | Error::WindowUnavailable => {
                            "Text input paused · native window unavailable"
                        }
                    },
                );
            }
        }
    }

    pub(crate) fn native_retirement_status(&self) -> Option<&'static str> {
        match self.retirement.receipt.as_ref()?.status() {
            Status::Pending => Some("Text input paused · native replacement pending"),
            Status::WindowClosed => Some("Text input paused · native window closed"),
            Status::Replaced {
                first_responder: false,
                responder_requested: true,
            } => Some("Text input paused · keyboard focus unavailable"),
            Status::Replaced { .. } => None,
        }
    }
}
