//! P13-scoped native-client handoff. A receipt is not queued-OS-input proof.
use crate::{app::Smoke, live::Live, terminal_input::Destination};
use gpui::{Context, Window};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Owner {
    host: String,
    selection: u64,
    destination: Destination,
    revision: u64,
    inbox: Arc<Mutex<Live>>,
}
impl PartialEq for Owner {
    fn eq(&self, other: &Self) -> bool {
        self.host == other.host
            && self.selection == other.selection
            && self.destination == other.destination
            && self.revision == other.revision
            && Arc::ptr_eq(&self.inbox, &other.inbox)
    }
}
#[derive(Clone, PartialEq)]
pub(crate) struct Ticket {
    serial: u64,
    retired: u64,
    owner: Owner,
}
#[derive(Default)]
pub(crate) struct Handoff {
    pub paused: bool,
    pub needs_request: bool,
    pub reason: Option<&'static str>,
    /// Counts `Smoke` renders. GPUI takes the platform input handler at the
    /// start of every `Window::draw` and installs only handlers registered in
    /// that frame (gpui-pre 0.3.12 window.rs:3685-3727); the root view renders
    /// inside that draw. A retired field stamped with this count is still the
    /// platform handler until the count moves.
    pub frames: u64,
    /// `frames` when an accepted acknowledgement reopened the fence. GPUI
    /// re-evaluates text input focus only at the end of `Window::draw`
    /// (gpui-pre 0.3.12 window.rs:3733-3740), so readiness is settled once a
    /// draw has started after this value.
    acked_frame: Option<u64>,
    serial: u64,
    pending: Option<Ticket>,
    #[cfg(target_os = "ios")]
    pub receipt: Option<herdr_uikit::TextInputRetirement>,
}
impl Smoke {
    pub(crate) fn find_keyboard_status(&self) -> Option<&'static str> {
        if let Some(reason) = self.find_handoff.reason {
            return Some(reason);
        }
        #[cfg(target_os = "ios")]
        if let Some(receipt) = &self.find_handoff.receipt {
            use herdr_uikit::TextInputRetirementStatus as Status;
            return match receipt.status() {
                Status::Pending => Some("Text input paused · keyboard replacement pending"),
                Status::WindowClosed => Some("Text input paused · keyboard window closed"),
                Status::Replaced {
                    first_responder: false,
                    responder_requested: true,
                } => Some("Text input paused · keyboard focus unavailable"),
                Status::Replaced { .. } => None,
            };
        }
        None
    }
    /// Whether a retired search field stamped at `frame` may hand its native
    /// callbacks to the terminal input path: search closed, the retirement
    /// fence open, the terminal shown, and no draw since the field retired.
    pub(crate) fn inherits_retired_field(&self, frame: u64) -> bool {
        self.find.is_none()
            && !self.find_handoff.paused
            && self.terminal.open
            && self.find_handoff.frames == frame
    }
    /// Whether GPUI has drawn since the accepted retirement acknowledgement.
    /// A draw while the fence was closed reported FocusLost
    /// (`accepts_text_input`, input.rs:98); the native client's third
    /// readiness value (herdr-uikit 33e288d text_input/probe.rs:54-60) holds
    /// that until the next draw re-evaluates focus. Read by the P13 probes.
    #[cfg_attr(
        not(any(test, feature = "p13-native-probe")),
        allow(
            dead_code,
            reason = "only the opt-in P13 probes and host tests read it"
        )
    )]
    pub(crate) fn find_input_settled(&self) -> bool {
        !self.find_handoff.paused
            && self
                .find_handoff
                .acked_frame
                .is_some_and(|frame| frame != self.find_handoff.frames)
    }
    pub(crate) fn fence_find_handoff(&mut self) {
        self.find_handoff.paused = true;
        self.find_handoff.acked_frame = None;
        self.find_handoff.needs_request = true;
        self.find_handoff.pending = None;
        self.find_handoff.reason = Some("Text input paused · retiring search keyboard");
    }
    fn find_input_owner(&self) -> Option<Owner> {
        if self.find.is_some()
            || !self.terminal.open
            || self.terminal.reconnect.suspended
            || self.terminal.navigating.is_some()
            || self.actions.palette
            // The settings sheet, not the settings store: `settings` holds the
            // SettingsHost loaded at startup and is Some for the app's lifetime.
            || self.settings_modal
        {
            return None;
        }
        let live = self.terminal.link.inbox.try_lock().ok()?;
        let destination = crate::terminal_input::destination(&live, self.terminal.link.generation)?;
        if self.terminal.input.destination.as_ref() != Some(&destination) {
            return None;
        }
        Some(Owner {
            host: self.terminal.reconnect.target.clone(),
            selection: self.board.selection,
            destination,
            revision: self.terminal.input.retirement_revision(),
            inbox: self.terminal.link.inbox.clone(),
        })
    }
    pub(crate) fn begin_find_retirement(&mut self, retired: u64) -> Option<Ticket> {
        if !self.find_handoff.paused {
            return None;
        }
        let owner = self.find_input_owner()?;
        let Some(serial) = self.find_handoff.serial.checked_add(1) else {
            self.find_handoff.reason = Some("Text input paused · keyboard identity exhausted");
            return None;
        };
        let ticket = Ticket {
            serial,
            retired,
            owner,
        };
        self.find_handoff.serial = serial;
        self.find_handoff.pending = Some(ticket.clone());
        Some(ticket)
    }
    pub(crate) fn complete_find_retirement(
        &mut self,
        ticket: &Ticket,
        retired: u64,
        replacement: u64,
        window: &Window,
    ) -> bool {
        if self.find_handoff.pending.as_ref() != Some(ticket) {
            return false;
        }
        self.find_handoff.pending = None;
        if !self.terminal_focus.is_focused(window)
            || self.find_input_owner().as_ref() != Some(&ticket.owner)
            || retired != ticket.retired
            || retired.checked_add(1) != Some(replacement)
        {
            self.find_handoff.reason = Some("Text input paused · keyboard owner changed");
            return false;
        }
        if !self
            .terminal
            .input
            .acknowledge_retirement(ticket.owner.revision)
        {
            self.find_handoff.reason = Some("Text input paused · composition changed");
            return false;
        }
        self.find_handoff.paused = false;
        self.find_handoff.acked_frame = Some(self.find_handoff.frames);
        self.find_handoff.reason = None;
        true
    }
    #[cfg(target_os = "ios")]
    pub(crate) fn request_find_retirement(&mut self, window: &Window, cx: &Context<Self>) {
        use herdr_uikit::TextInputRetirementError as Error;
        if !std::mem::take(&mut self.find_handoff.needs_request) {
            return;
        }
        let handle = window.window_handle();
        let Some(state) = herdr_uikit::text_input_session_state(handle) else {
            self.find_handoff.reason = Some("Text input paused · keyboard unavailable");
            self.observation.record(
                serde_json::json!({"kind":"p13-retirement-unrequested", "reason":"session-state"}),
            );
            return;
        };
        let Some(ticket) = self.begin_find_retirement(state.client) else {
            self.find_handoff.reason = Some("Text input paused · destination unavailable");
            self.observation
                .record(serde_json::json!({"kind":"p13-retirement-unrequested", "reason":"owner"}));
            return;
        };
        let context = window.to_async(cx);
        let weak = cx.entity().downgrade();
        let callback_ticket = ticket.clone();
        self.find_handoff.receipt = None;
        let result = herdr_uikit::retire_text_input_session(handle, move |sessions| {
            deliver_find_retirement(
                context,
                weak,
                handle,
                callback_ticket,
                (sessions.retired, sessions.replacement),
            );
        });
        match result {
            Ok(receipt) => self.find_handoff.receipt = Some(receipt),
            Err(error) => {
                self.find_handoff.pending = None;
                // A close from inside a UIKit callback (the field's Escape) is
                // refused only until that callback returns: submit once more
                // from the next render. Other refusals stay fenced.
                self.find_handoff.needs_request = error == Error::NativeCallbackInFlight;
                self.find_handoff.reason = Some(match error {
                    Error::NativeCallbackInFlight => {
                        "Text input paused · keyboard callback still active"
                    }
                    Error::InProgress => "Text input paused · keyboard replacement pending",
                    Error::IdentityExhausted => "Text input paused · keyboard identity exhausted",
                    Error::NotMainThread | Error::WindowUnavailable => {
                        "Text input paused · keyboard unavailable"
                    }
                });
                self.observation.record(serde_json::json!({"kind":"p13-retirement-refused", "serial":ticket.serial,"reason":format!("{error:?}")}));
            }
        }
    }
    #[cfg(not(target_os = "ios"))]
    pub(crate) fn request_find_retirement(&mut self, _: &Window, _: &Context<Self>) {
        self.find_handoff.needs_request = false;
        self.find_handoff.reason = Some("Text input paused · native keyboard unavailable");
    }
    /// Host fixture supplies the boundary explicitly; never built into the app.
    #[cfg(test)]
    pub(crate) fn controlled_find_retirement(&mut self, window: &Window) {
        let ticket = self
            .begin_find_retirement(40)
            .unwrap_or_else(|| panic!("D5 active final owner"));
        assert!(
            self.complete_find_retirement(&ticket, 40, 41, window),
            "D5 correlated retirement rejected"
        );
    }
}

/// Runs the SDK retirement callback body once per accepted request.
///
/// The SDK drains its callback when no SDK-entered GPUI call is on the stack.
/// A turn GPUI did not enter (an SDK `next_native_turn` probe) can still hold
/// GPUI's app borrowed through `AsyncWindowContext::update`; the immediate
/// update then fails instead of acknowledging, and dropping that error left
/// the fence paused for good. One foreground task acknowledges instead; the
/// fence keeps input closed until it runs.
#[cfg(any(target_os = "ios", test))]
pub(crate) fn deliver_find_retirement(
    mut context: gpui::AsyncWindowContext,
    weak: gpui::WeakEntity<Smoke>,
    handle: gpui::AnyWindowHandle,
    ticket: Ticket,
    sessions: (u64, u64),
) {
    let immediate = context.update(|window, cx| {
        acknowledge_find_retirement(&weak, handle, &ticket, sessions, false, window, cx)
    });
    if immediate.is_ok() {
        return;
    }
    context
        .spawn(async move |cx| {
            let _ = cx.update(|window, cx| {
                acknowledge_find_retirement(&weak, handle, &ticket, sessions, true, window, cx)
            });
        })
        .detach();
}

#[cfg(any(target_os = "ios", test))]
fn acknowledge_find_retirement(
    weak: &gpui::WeakEntity<Smoke>,
    handle: gpui::AnyWindowHandle,
    ticket: &Ticket,
    (retired, replacement): (u64, u64),
    deferred: bool,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let _ = weak.update(cx, |this, cx| {
        let accepted = window.window_handle() == handle
            && this.complete_find_retirement(ticket, retired, replacement, window);
        if accepted {
            // Request one frame now. An ack that lands after the first
            // post-close draw would otherwise leave GPUI reporting no text
            // input focus until some unrelated invalidation draws again.
            window.refresh();
        }
        this.observation.record(serde_json::json!({
            "kind":"p13-native-retirement", "accepted":accepted, "deferred":deferred,
            "serial":ticket.serial, "retired":retired, "replacement":replacement
        }));
        cx.notify();
    });
}
