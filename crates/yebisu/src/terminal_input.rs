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
    pub boot: String,
    pub target: InputTarget,
}

pub(crate) fn destination(live: &Live, generation: u64) -> Option<Destination> {
    if !live.connected || live.version_mismatch.is_some() {
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
        boot: snapshot.boot_id.clone(),
        target,
    })
}

#[derive(Default)]
pub(crate) struct InputOptions {
    pub option_alt: bool,
    pub command_control: bool,
    pub escape_cancels: bool,
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
    pub ready: bool,
}

impl TerminalInput {
    pub fn sync(&mut self, live: &Live, generation: u64, size: Option<ConnectOptions>) {
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
    pub fn discard(&mut self) {
        self.cancelled |= !self.composition.text.is_empty();
        self.composition = Input::default();
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
    pub fn commit(&mut self, text: &str) -> Option<ClientPaneInputEvent> {
        self.composition = Input::default();
        if self.cancelled || self.destination.is_none() || text.is_empty() {
            return None;
        }
        Some(ClientPaneInputEvent::TextCommit(text.into()))
    }
    pub fn key_down(
        &mut self,
        event: &KeyDownEvent,
        report_all: bool,
    ) -> Option<ClientPaneInputEvent> {
        self.held.forget(&event.keystroke.key);
        self.fresh_gesture();
        self.destination.as_ref()?;
        if !self.composition.text.is_empty() {
            if event.keystroke.key == "escape" && self.options.escape_cancels {
                self.discard();
            } else {
                return None;
            }
        }
        let mut mapped = event.clone();
        if self.options.command_control && mapped.keystroke.modifiers.platform {
            mapped.keystroke.modifiers.platform = false;
            mapped.keystroke.modifiers.control = true;
        }
        let event = crate::terminal::key_input(&mapped, self.options.option_alt)?;
        Some(self.held.press(&mapped.keystroke.key, event, report_all))
    }
    pub fn queue(&mut self, event: ClientPaneInputEvent) -> bool {
        let Some(target) = &self.destination else {
            return false;
        };
        self.pending
            .get_or_insert_with(|| HeldInput::new(target.boot.clone(), target.target.clone()))
            .push(event)
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
