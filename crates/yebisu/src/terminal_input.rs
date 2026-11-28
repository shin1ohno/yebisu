//! Semantic input scoped to the currently displayed connection and target.
//! Modeled on sheprd-gpui input.rs at 398c132a0, not core_terminals/view.rs.
use crate::{
    input::Input,
    link::HeldInput,
    live::Live,
    terminal::{HeldKeys, InputTarget},
};
use gpui::KeyDownEvent;
use sheprd_client::{ConnectOptions, protocol::ClientPaneInputEvent};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Destination {
    pub generation: u64,
    pub epoch: u64,
    pub boot: String,
    pub target: InputTarget,
}

pub(crate) fn destination(live: &Live, generation: u64) -> Option<Destination> {
    if !live.connected || live.version_mismatch.get().is_some() {
        return None;
    }
    let snapshot = live.snapshot.as_ref()?;
    let surface = live.surface.as_ref()?;
    if !crate::live::coherent(snapshot, surface) {
        return None;
    }
    let target = match &surface.popup {
        Some(popup) => InputTarget::Popup(popup.terminal_id.clone()),
        None => InputTarget::Pane(snapshot.focused_pane_id.clone()?),
    };
    Some(Destination {
        generation,
        epoch: live.input_epoch,
        boot: snapshot.boot_id.clone(),
        target,
    })
}

/// Why `destination` is `None`, for diagnostics only; `destination` decides.
pub(crate) fn gap(live: &Live) -> Option<&'static str> {
    if !live.connected || live.version_mismatch.get().is_some() {
        return Some("not-connected");
    }
    let (Some(snapshot), Some(surface)) = (live.snapshot.as_ref(), live.surface.as_ref()) else {
        return Some("no-destination");
    };
    if !crate::live::coherent(snapshot, surface) {
        return Some("incoherent");
    }
    match &surface.popup {
        Some(_) => None,
        None => snapshot
            .focused_pane_id
            .is_none()
            .then_some("no-destination"),
    }
}

/// A semantic event keeps the identity observed when it was generated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedInput {
    pub destination: Destination,
    pub event: ClientPaneInputEvent,
}

#[derive(Default)]
pub(crate) struct InputOptions {
    pub option_alt: bool,
    pub command_control: bool,
    pub caret_anchor: bool,
}

#[derive(Default)]
pub(crate) struct TerminalInput {
    pub composition: Input,
    pub destination: Option<Destination>,
    pub held: HeldKeys,
    pub options: InputOptions,
    pending: Option<HeldInput>,
    cancelled: bool,
    /// The fence holds no discarded composition (the settings sheet), so a
    /// fresh key press may release it; a discard fence never sets this.
    key_resumes: bool,
    retirement_revision: u64,
    pub ready: bool,
    pub report_all: bool,
    gap: Option<&'static str>,
    reported: std::cell::Cell<Option<&'static str>>,
    /// The window is too small for one pane (L9, Q1): nothing is queued or
    /// sent until it grows again.
    size_hold: bool,
    /// A native field closed under the user (#270: the rename field after a
    /// boot change): no key or text reaches the terminal until a terminal
    /// gesture (`fresh_gesture`). Unlike a discard fence, native retirement
    /// never ends it, and a detach, a sync or a reconnect keeps it.
    tap_hold: Option<&'static str>,
}

impl TerminalInput {
    pub fn sync(&mut self, live: &Live, generation: u64, size: Option<ConnectOptions>) {
        self.report_all = live.keyboard_report_all;
        let next = destination(live, generation);
        self.gap = gap(live);
        if next.is_some() {
            self.reported.set(None);
        }
        if self.destination != next {
            self.discard();
            self.held = HeldKeys::default();
            self.pending = None;
            self.destination = next;
        }
        self.ready = self.destination.is_some()
            && size.is_none_or(|size| {
                live.surface.as_ref().is_some_and(|s| {
                    s.frame.width == size.surface_size.cols
                        && s.frame.height == size.surface_size.rows
                })
            });
    }
    pub fn accepts(&self, focused: bool, setup: bool) -> bool {
        focused && !setup && self.destination.is_some()
    }
    /// Drop the destination and queued keys without inventing a composition.
    /// An actual discarded composition remains fenced across repeated detaches.
    pub fn detach(&mut self) {
        self.discard();
        // P13 owners compare this revision: any detach invalidates their ticket.
        let revision = self.retirement_revision.saturating_add(1);
        let cancelled = self.cancelled;
        let key_resumes = self.key_resumes;
        let size_hold = self.size_hold;
        let tap_hold = self.tap_hold;
        let options = std::mem::take(&mut self.options);
        *self = Self::default();
        self.options = options;
        self.cancelled = cancelled;
        self.key_resumes = key_resumes;
        self.retirement_revision = revision;
        self.size_hold = size_hold;
        self.tap_hold = tap_hold;
    }
    /// L9: while `hold` is true the window is too small for one pane (Q1,
    /// D2), so no input reaches the terminal. Entering the hold drops what
    /// is queued, so nothing typed into a too-small window leaves later.
    /// Whether the window is too small for one pane, so nothing is sent.
    pub fn holds_size(&self) -> bool {
        self.size_hold
    }
    pub fn hold_for_size(&mut self, hold: bool) {
        if hold && !self.size_hold {
            self.pending = None;
            self.held = HeldKeys::default();
        }
        self.size_hold = hold;
    }
    /// #270: hold keys and text away from the terminal until it is tapped,
    /// for `reason` (the closed field's status text). What is queued or held
    /// is dropped, so nothing typed before the hold leaves later either.
    pub fn hold_until_tap(&mut self, reason: &'static str) {
        self.discard();
        self.pending = None;
        self.held = HeldKeys::default();
        self.tap_hold = Some(reason);
    }
    /// Why keys and text wait for a terminal tap, if they do.
    pub fn held_until_tap(&self) -> Option<&'static str> {
        self.tap_hold
    }
    /// The native text client is shared across hosts, unlike terminal queues.
    pub fn retain_discard(&mut self, cancelled: bool) {
        if cancelled && !self.cancelled {
            self.record_discard();
        }
    }
    /// A boundary that took text input away (the settings sheet) fences late
    /// native commits until the next fresh key press. It holds no discarded
    /// composition, so unlike a discard a new press releases it before UIKit
    /// types the key (SH1-156 review F1); a held key from the sheet does not.
    pub fn fence(&mut self) {
        if !self.cancelled {
            self.record_discard();
            self.key_resumes = true;
        }
    }
    /// A rejected marked callback can still have a later native commit.
    pub fn reject_mark(&mut self, text: &str) {
        self.discard();
        if !text.is_empty() {
            self.record_discard();
        }
    }
    pub fn discard(&mut self) {
        if !self.composition.text.is_empty() {
            self.record_discard();
        }
        self.composition = Input::default();
    }
    pub fn retirement_revision(&self) -> u64 {
        self.retirement_revision
    }
    /// Records once per reason why typed text cannot reach a terminal. Without it a
    /// positive control that never arrives looks the same as no input at all.
    pub fn report_drop(&self, observation: &crate::observation::Observation) {
        let reason = if self.tap_hold.is_some() {
            Some("held-for-tap")
        } else if self.destination.is_none() {
            Some(self.gap.unwrap_or("no-destination"))
        } else if self.cancelled {
            Some("fenced")
        } else {
            None
        };
        if self.reported.get() == reason {
            return;
        }
        self.reported.set(reason);
        if let Some(reason) = reason {
            observation.record(serde_json::json!({"kind":"input-dropped", "reason":reason}));
        }
    }
    pub fn requires_gesture(&self) -> bool {
        self.cancelled
    }
    fn record_discard(&mut self) {
        self.cancelled = true;
        self.key_resumes = false;
        self.retirement_revision = self.retirement_revision.saturating_add(1);
    }
    /// Called only after a correlated native client retirement callback.
    /// The revision must match: a detach, discard or rejected mark since the
    /// request invalidates it. Without a discard fence (a P13 search close with
    /// no terminal composition) a matching revision has nothing to release and
    /// still settles the owner. Saturation is permanently closed: an old
    /// receipt must never match again.
    pub fn acknowledge_retirement(&mut self, revision: u64) -> bool {
        if revision != self.retirement_revision || revision == u64::MAX {
            return false;
        }
        if self.cancelled {
            self.composition = Input::default();
            self.cancelled = false;
            self.key_resumes = false;
        }
        true
    }
    /// The native search panel closed and UIKit gave the text view first
    /// responder back. Same rule as a retirement receipt: only a revision
    /// unchanged since the search opened releases a discard fence.
    pub fn release_after_native_search(&mut self, revision: u64) -> bool {
        self.acknowledge_retirement(revision)
    }
    /// An explicit terminal gesture (Paste, Select off, a tap on the terminal)
    /// starts new input under the settled P8 contract (docs/p8-contract.md).
    /// A hardware key press never calls this: only native retirement releases
    /// a discard fence without a gesture (SH1-156).
    pub fn fresh_gesture(&mut self) {
        self.cancelled = false;
        self.key_resumes = false;
        self.tap_hold = None;
    }
    pub fn mark(&mut self, text: &str, selected: Option<std::ops::Range<usize>>) {
        if self.destination.is_none() || self.cancelled || self.tap_hold.is_some() {
            self.reject_mark(text);
            return;
        }
        self.composition = Input::default();
        self.composition.replace(None, text, true, selected);
    }
    pub fn replace(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
    ) -> Option<GeneratedInput> {
        if let (Some(range), Some(marked)) = (&range, &self.composition.marked)
            && range.start <= range.end
            && marked.start <= range.start
            && range.end <= marked.end
            && range != marked
        {
            // UIKit selection deletion/replacement edits uncommitted text.
            self.composition
                .replace(Some(range.clone()), text, false, None);
            let end = self.composition.text.encode_utf16().count();
            self.composition.marked = (end > 0).then_some(0..end);
            return None;
        }
        self.commit(text)
    }
    pub fn commit(&mut self, text: &str) -> Option<GeneratedInput> {
        self.composition = Input::default();
        if self.cancelled || self.tap_hold.is_some() || self.destination.is_none() || text.is_empty()
        {
            return None;
        }
        self.generated(ClientPaneInputEvent::TextCommit(text.into()))
    }
    pub fn key_down(&mut self, event: &KeyDownEvent, report_all: bool) -> Option<GeneratedInput> {
        self.held.forget(&event.keystroke.key);
        // A key press cannot acknowledge native composition cancellation; it
        // only ends a fence that holds none (`fence`).
        if self.key_resumes && !event.is_held {
            self.cancelled = false;
            self.key_resumes = false;
        }
        self.destination.as_ref()?;
        if self.tap_hold.is_some() || !self.composition.text.is_empty() {
            return None;
        }
        let mut mapped = event.clone();
        if self.options.command_control && mapped.keystroke.modifiers.platform {
            mapped.keystroke.modifiers.platform = false;
            mapped.keystroke.modifiers.control = true;
        }
        let event = crate::terminal::key_input(&mapped, self.options.option_alt)?;
        let event = self.held.press(&mapped.keystroke.key, event, report_all);
        self.generated(event)
    }
    fn generated(&self, event: ClientPaneInputEvent) -> Option<GeneratedInput> {
        Some(GeneratedInput {
            destination: self.destination.clone()?,
            event,
        })
    }
    pub fn key_up(&mut self, key: &str) -> Option<GeneratedInput> {
        let event = self.held.release(key)?;
        if !self.report_all {
            return None;
        }
        self.generated(event)
    }
    pub fn queue(&mut self, input: GeneratedInput) -> bool {
        if self.size_hold {
            return false;
        }
        let Some(target) = &self.destination else {
            return false;
        };
        if target != &input.destination {
            return false;
        }
        self.pending
            .get_or_insert_with(|| HeldInput::new(target.boot.clone(), target.target.clone()))
            .push(input.event)
    }
    pub fn next(&mut self) -> Option<ClientPaneInputEvent> {
        if !self.ready || self.size_hold {
            return None;
        }
        let target = self.destination.as_ref()?;
        self.pending.as_mut()?.pop(&target.boot, &target.target)
    }
    /// Read-only gate for the P13 probe's after-close barriers: nothing is
    /// held and the last sync found the surface at the requested size, so
    /// input generated so far has already left through `next`.
    #[cfg(any(
        test,
        all(target_os = "ios", target_abi = "sim", feature = "p13-native-probe")
    ))]
    pub fn flushed(&self) -> bool {
        self.ready && self.pending.as_ref().is_none_or(HeldInput::is_empty)
    }
}

#[cfg(test)]
#[path = "terminal_input_tests.rs"]
mod tests;
