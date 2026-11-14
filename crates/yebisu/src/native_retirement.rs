//! Limited SDK Edit retirement. Unsupported input remains fenced; never replay.
use crate::{app::Smoke, input_retirement::Ticket};
use gpui::{AsyncWindowContext, Context, WeakEntity, Window};
use herdr_uikit::{
    NativeInputOrigin, NativeRetirementRequestError as Error,
    NativeRetirementRequestStatus as Status, PrepareDecision,
};

#[derive(Clone)]
pub(crate) struct Binding {
    context: AsyncWindowContext,
    entity: WeakEntity<Smoke>,
}

impl Binding {
    fn update<R>(
        &mut self,
        change: impl FnOnce(&mut Smoke, &mut Window, &mut Context<Smoke>) -> R,
    ) -> Option<R> {
        let entity = self.entity.clone();
        self.context
            .update(|window, cx| entity.update(cx, |this, cx| change(this, window, cx)))
            .ok()?
            .ok()
    }
}

impl Smoke {
    pub(crate) fn bind_text_retirement(&mut self, window: &Window, cx: &Context<Self>) {
        self.retirement.native = Some(Binding {
            context: window.to_async(cx),
            entity: cx.entity().downgrade(),
        });
    }

    fn native_owner_matches(&self, ticket: &Ticket, window: &Window) -> bool {
        self.text_retirement_matches(ticket)
            && self
                .retirement
                .native
                .as_ref()
                .is_some_and(|binding| binding.context.window_handle() == window.window_handle())
            && self.terminal_focus.is_focused(window)
    }

    pub(crate) fn request_text_retirement(&mut self) {
        if let Some(receipt) = self.retirement.receipt.clone()
            && matches!(receipt.status(), Status::Queued { .. } | Status::Preparing)
        {
            // Cancellation first invalidates the consumer ticket. Receipt Drop
            // alone does not cancel; never register a second ticket while queued.
            if !self.retirement_owner_current() {
                self.cancel_text_retirement();
                let _ = receipt.mark_owner_cancelled();
            }
            return;
        }
        if !self.terminal.input.requires_gesture() {
            return;
        }
        let Some(mut binding) = self.retirement.native.clone() else {
            self.retirement.reason = Some("Text input paused · native window unavailable");
            return;
        };
        let handle = binding.context.window_handle();
        let Some(origin) = herdr_uikit::native_input_origin(handle) else {
            self.retirement.reason = Some("Text input paused · native window unavailable");
            return;
        };
        let Some(ticket) = self.begin_text_retirement(origin.client()) else {
            return;
        };
        self.retirement.receipt = None;
        self.retirement.key = None;
        let callback_ticket = ticket.clone();
        let result =
            herdr_uikit::request_native_edit_retirement(handle, origin.clone(), move |prepared| {
                let accepted = binding
                    .update(|this, window, cx| {
                        let accepted = !prepared.owner_cancelled
                            && prepared.old_origin == origin
                            && prepared.key.origin() == &origin
                            && herdr_uikit::native_input_origin(handle).as_ref() == Some(&origin)
                            && this.native_owner_matches(&callback_ticket, window)
                            && this.prepare_text_retirement(&callback_ticket);
                        if accepted {
                            this.retirement.key = Some(prepared.key.clone());
                        } else {
                            this.refuse_text_retirement(
                                &callback_ticket,
                                "Text input paused · retirement owner changed",
                            );
                        }
                        this.observation.record(serde_json::json!({
                            "kind":"native-retirement-prepare", "serial":callback_ticket.serial,
                            "sdk_serial":prepared.key.serial(), "old":origin.client(),
                            "accepted":accepted, "fenced":this.terminal.input.requires_gesture()
                        }));
                        cx.notify();
                        accepted
                    })
                    .unwrap_or(false);
                if !accepted {
                    return PrepareDecision::Decline;
                }
                let key = prepared.key;
                PrepareDecision::Proceed(Box::new(move |outcome| {
                    let _ = binding.update(|this, window, cx| {
                        let ready = match &outcome.status {
                            Status::Replaced {
                                successor,
                                responder_requested: true,
                                first_responder: true,
                            } => {
                                successor != &origin
                                    && herdr_uikit::native_input_origin(handle).as_ref()
                                        == Some(successor)
                                    && native_ready(handle, successor)
                            }
                            _ => false,
                        };
                        let accepted = ready
                            && outcome.key == key
                            && this.retirement.key.as_ref() == Some(&key)
                            && this.native_owner_matches(&callback_ticket, window)
                            && this.acknowledge_text_retirement(&callback_ticket);
                        if !accepted {
                            this.refuse_text_retirement(
                                &callback_ticket,
                                "Text input paused · replacement not ready or owner changed",
                            );
                        }
                        this.observation.record(serde_json::json!({
                            "kind":"native-retirement-completed", "serial":callback_ticket.serial,
                            "sdk_serial":key.serial(), "status":format!("{:?}", outcome.status),
                            "accepted":accepted
                        }));
                        cx.notify();
                    });
                }))
            });
        match result {
            Ok(receipt) => self.retirement.receipt = Some(receipt),
            Err(error) => {
                self.observation.record(serde_json::json!({
                    "kind":"native-retirement-refused", "serial":ticket.serial,
                    "reason":format!("{error:?}")
                }));
                self.refuse_text_retirement(
                    &ticket,
                    match error {
                        Error::UnsupportedHardwareContext => {
                            "Text input paused · hardware recovery unsupported by SDK"
                        }
                        Error::UnsupportedNativeEntry
                        | Error::UnsupportedReentrancy
                        | Error::UnprotectedEntry => {
                            "Text input paused · this native entry cannot retire composition"
                        }
                        Error::InProgress
                        | Error::NewCapacityBusy
                        | Error::LegacyDispatchActive => {
                            "Text input paused · native retirement busy"
                        }
                        Error::IdentityExhausted => "Text input paused · native identity exhausted",
                        Error::StaleOrigin => "Text input paused · native owner changed",
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
            Status::Queued { .. } | Status::Preparing => {
                Some("Text input paused · native replacement pending")
            }
            Status::WindowClosed { .. } => Some("Text input paused · native window closed"),
            Status::Replaced {
                first_responder: true,
                responder_requested: true,
                ..
            } => None,
            _ => Some("Text input paused · native replacement not ready"),
        }
    }
}

fn native_ready(handle: gpui::AnyWindowHandle, successor: &NativeInputOrigin) -> bool {
    herdr_uikit::text_input_session_state(handle).is_some_and(|state| {
        state.client == successor.client()
            && !state.retired
            && !state.replacement_pending
            && state.attached
            && state.first_responder
    })
}
