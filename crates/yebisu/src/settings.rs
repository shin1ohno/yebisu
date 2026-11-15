//! Container-local draft ownership. Disk work is serialized off the UI thread.
pub(crate) mod font;
pub(crate) mod key_ops;
mod store;
mod worker;

use crate::Result;
#[cfg(target_os = "ios")]
use herdr_uikit::BackgroundTask;
#[cfg(all(test, not(target_os = "ios")))]
use retry_tests::BackgroundTask;
use std::{path::PathBuf, sync::mpsc};
use worker::{Completion, Request};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Values {
    pub theme: crate::theme_picker::Palette,
    pub font: font::TerminalFont,
    pub compact: bool,
    pub terminal_priority: bool,
    pub board_hidden: bool,
    pub drag_select: bool,
    pub copy_on_select: bool,
}

impl Default for Values {
    fn default() -> Self {
        Self {
            font: font::TerminalFont::default(),
            theme: crate::theme_picker::Palette::default(),
            compact: false,
            terminal_priority: false,
            board_hidden: false,
            drag_select: true,
            copy_on_select: false,
        }
    }
}

// Edit tracking is independent of setting defaults (Drag defaults to true).
#[derive(Default)]
struct Edited {
    theme: bool,
    font_size: bool,
    font_family: bool,
    compact: bool,
    terminal_priority: bool,
    board_hidden: bool,
    drag_select: bool,
    copy_on_select: bool,
}

pub(crate) struct SettingsHost {
    pub keys: key_ops::Owner,
    pub draft: Values,
    pub error: Option<String>,
    revision: u64,
    saved: u64,
    loaded: bool,
    edited_before_load: Edited,
    blocked: bool,
    write_failed: bool,
    // Consumed only when a save is sent, never by an older completion.
    retry_requested: bool,
    in_flight: Option<u64>,
    sender: mpsc::SyncSender<Request>,
    receiver: mpsc::Receiver<Completion>,
    #[cfg(any(target_os = "ios", test))]
    background: Option<BackgroundTask>,
    // P11 owns the confirmation UI; no regeneration is executed in P10.
    regeneration_confirmed: bool,
}
impl SettingsHost {
    pub fn open(root: PathBuf) -> Result<Self> {
        let (sender, receiver) = worker::start(root)?;
        Ok(Self {
            keys: key_ops::Owner::default(),
            draft: Values::default(),
            error: None,
            revision: 0,
            saved: 0,
            loaded: false,
            edited_before_load: Edited::default(),
            blocked: false,
            write_failed: false,
            retry_requested: false,
            in_flight: None,
            sender,
            receiver,
            #[cfg(any(target_os = "ios", test))]
            background: None,
            regeneration_confirmed: false,
        })
    }

    pub fn choose_theme(&mut self, palette: crate::theme_picker::Palette) {
        if !self.loaded {
            self.edited_before_load.theme = true;
            if self.draft.theme == palette {
                self.revision += 1;
                self.retry_requested = true;
            }
        }
        let mut values = self.draft.clone();
        values.theme = palette;
        self.edit(values);
    }

    pub fn edit(&mut self, values: Values) {
        self.edit_values(values, false);
    }

    // A density gesture resets both fields, even when the pending draft already
    // equals Regular. Polling snapshots must never acquire this explicit intent.
    pub(crate) fn edit_density(&mut self, values: Values) {
        self.edit_values(values, true);
    }

    fn edit_values(&mut self, values: Values, density_selected: bool) {
        if let Err(error) = values.font.validate() {
            self.error = Some(error.to_string());
            return;
        }
        let pending_density = density_selected && !self.loaded;
        if pending_density {
            self.edited_before_load.compact = true;
            self.edited_before_load.font_size = true;
        }
        if self.draft != values || pending_density {
            if !self.loaded {
                self.edited_before_load.theme |= self.draft.theme != values.theme;
                self.edited_before_load.font_size |= self.draft.font.size != values.font.size;
                self.edited_before_load.font_family |= self.draft.font.family != values.font.family;
                self.edited_before_load.compact |= self.draft.compact != values.compact;
                self.edited_before_load.terminal_priority |=
                    self.draft.terminal_priority != values.terminal_priority;
                self.edited_before_load.board_hidden |=
                    self.draft.board_hidden != values.board_hidden;
            }
            if !self.loaded {
                self.edited_before_load.drag_select |= self.draft.drag_select != values.drag_select;
                self.edited_before_load.copy_on_select |=
                    self.draft.copy_on_select != values.copy_on_select;
            }
            self.write_failed = false;
            self.retry_requested = true;
            self.draft = values;
            self.revision += 1;
            self.regeneration_confirmed = false;
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut applied = false;
        while let Ok(reply) = self.receiver.try_recv() {
            match reply {
                Completion::Loaded(result) => {
                    self.loaded = true;
                    match result {
                        Ok(values) => {
                            if !self.edited_before_load.theme {
                                self.draft.theme = values.theme;
                            }
                            if !self.edited_before_load.font_size {
                                self.draft.font.size = values.font.size;
                            }
                            if !self.edited_before_load.font_family {
                                self.draft.font.family = values.font.family;
                            }
                            if !self.edited_before_load.compact {
                                self.draft.compact = values.compact;
                            }
                            if !self.edited_before_load.terminal_priority {
                                self.draft.terminal_priority = values.terminal_priority;
                            }
                            if !self.edited_before_load.board_hidden {
                                self.draft.board_hidden = values.board_hidden;
                            }
                            if !self.edited_before_load.drag_select {
                                self.draft.drag_select = values.drag_select;
                            }
                            if !self.edited_before_load.copy_on_select {
                                self.draft.copy_on_select = values.copy_on_select;
                            }
                            applied = true;
                        }
                        Err(error) => {
                            self.blocked = true;
                            self.error = Some(error.to_string());
                        }
                    }
                }
                Completion::Saved(revision, result) => self.complete(revision, result),
            }
        }
        self.flush();
        #[cfg(any(target_os = "ios", test))]
        if self.in_flight.is_none() && (self.ready() || self.blocked || self.write_failed) {
            self.background.take();
        }
        applied
    }

    fn complete(&mut self, revision: u64, result: Result<()>) {
        if self.in_flight != Some(revision) {
            return;
        }
        self.in_flight = None;
        match result {
            Ok(()) => {
                self.saved = revision;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.write_failed = !self.retry_requested;
            }
        }
    }

    pub fn flush(&mut self) {
        if !self.loaded
            || self.blocked
            || self.write_failed
            || self.in_flight.is_some()
            || self.saved == self.revision
        {
            return;
        }
        match self.sender.try_send(Request::Save {
            revision: self.revision,
            values: self.draft.clone(),
        }) {
            Ok(()) => {
                self.in_flight = Some(self.revision);
                self.retry_requested = false;
            }
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.error = Some("settings worker disconnected".into());
                self.blocked = true;
            }
        }
    }

    pub fn lifecycle(&mut self, phase: gpui::AppLifecyclePhase) {
        if matches!(
            phase,
            gpui::AppLifecyclePhase::Inactive
                | gpui::AppLifecyclePhase::Background
                | gpui::AppLifecyclePhase::Disconnected
        ) {
            #[cfg(any(target_os = "ios", test))]
            if !self.ready() && !self.blocked && self.background.is_none() {
                self.background = BackgroundTask::begin("Saving yebisu settings");
            }
            self.flush();
        }
        if phase == gpui::AppLifecyclePhase::Active {
            self.write_failed = false;
            self.retry_requested = true;
            self.flush();
        }
    }

    pub fn retry(&mut self) {
        self.write_failed = false;
        self.retry_requested = true;
        self.flush();
    }

    pub fn ready(&self) -> bool {
        self.loaded && !self.blocked && self.saved == self.revision
    }

    #[allow(dead_code, reason = "P11 confirmation UI consumes this foundation")]
    pub fn confirm_regeneration(&mut self) {
        self.regeneration_confirmed = true;
    }
    #[allow(
        dead_code,
        reason = "P11 key worker must consume explicit confirmation once"
    )]
    pub fn take_regeneration_confirmation(&mut self) -> bool {
        std::mem::take(&mut self.regeneration_confirmed)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod retry_tests;

#[cfg(test)]
mod selection_tests;

#[cfg(test)]
mod font_tests;

#[cfg(test)]
mod theme_tests;
