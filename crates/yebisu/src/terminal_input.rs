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
    retirement_revision: u64,
    pub ready: bool,
    pub report_all: bool,
}

impl TerminalInput {
    pub fn sync(&mut self, live: &Live, generation: u64, size: Option<ConnectOptions>) {
        self.report_all = live.keyboard_report_all;
        let next = destination(live, generation);
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
    /// Fence selection/API changes, including queued keys and late IME callbacks.
    pub fn detach(&mut self) {
        let revision = self.retirement_revision.saturating_add(1);
        let options = std::mem::take(&mut self.options);
        *self = Self::default();
        self.options = options;
        self.cancelled = true;
        self.retirement_revision = revision;
    }
    pub fn discard(&mut self) {
        if !self.composition.text.is_empty() {
            self.retirement_revision = self.retirement_revision.saturating_add(1);
        }
        self.cancelled |= !self.composition.text.is_empty();
        self.composition = Input::default();
    }
    pub fn retirement_revision(&self) -> u64 {
        self.retirement_revision
    }
    /// Only a correlated native-client retirement may release this revision.
    pub fn acknowledge_retirement(&mut self, revision: u64) -> bool {
        if !self.cancelled || revision != self.retirement_revision || revision == u64::MAX {
            return false;
        }
        self.composition = Input::default();
        self.cancelled = false;
        true
    }
    pub fn requires_gesture(&self) -> bool {
        self.cancelled
    }
    pub fn fresh_gesture(&mut self) {
        self.cancelled = false;
    }
    pub fn mark(&mut self, text: &str, selected: Option<std::ops::Range<usize>>) {
        if self.destination.is_none() || self.cancelled {
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
        if self.cancelled || self.destination.is_none() || text.is_empty() {
            return None;
        }
        self.generated(ClientPaneInputEvent::TextCommit(text.into()))
    }
    pub fn key_down(&mut self, event: &KeyDownEvent, report_all: bool) -> Option<GeneratedInput> {
        self.held.forget(&event.keystroke.key);
        self.fresh_gesture();
        self.destination.as_ref()?;
        if !self.composition.text.is_empty() {
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
        if !self.ready {
            return None;
        }
        let target = self.destination.as_ref()?;
        self.pending.as_mut()?.pop(&target.boot, &target.target)
    }
}

#[cfg(test)]
#[path = "terminal_input_tests.rs"]
mod tests;
