//! Container-local draft ownership. Disk work is serialized off the UI thread.
pub(crate) mod font;
pub(crate) mod input_options;
pub(crate) mod key_ops;
pub(crate) mod sidebar_density;
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
    pub notifications: crate::notifications::Policy,
    pub theme: crate::theme_picker::Palette,
    pub font: font::TerminalFont,
    pub compact: bool,
    pub terminal_priority: bool,
    pub board_hidden: bool,
    pub drag_select: bool,
    pub copy_on_select: bool,
    // SH1-157 L1 (Q7, q7): Input options and Return, see input_options.rs.
    pub herdr_prefix: bool,
    pub use_server_keybindings: bool,
    pub option_alt: bool,
    pub command_control: bool,
    pub caret_anchor: bool,
    pub reconnect_automatic: bool,
    // Design fidelity plan Wave 0a: Settings › Sidebar, see sidebar_density.rs.
    pub sidebar_density: sidebar_density::SidebarDensity,
}

impl Default for Values {
    fn default() -> Self {
        Self {
            notifications: Default::default(),
            font: font::TerminalFont::default(),
            theme: crate::theme_picker::Palette::default(),
            compact: false,
            terminal_priority: false,
            board_hidden: false,
            drag_select: true,
            copy_on_select: false,
            herdr_prefix: false,
            use_server_keybindings: true,
            option_alt: false,
            command_control: false,
            caret_anchor: false,
            reconnect_automatic: true,
            sidebar_density: sidebar_density::SidebarDensity::default(),
        }
    }
}

// Edit tracking is independent of setting defaults (Drag defaults to true).
#[derive(Default)]
struct Edited {
    notifications: crate::notifications::Edits,
    theme: bool,
    font_size: bool,
    font_family: bool,
    compact: bool,
    terminal_priority: bool,
    board_hidden: bool,
    drag_select: bool,
    copy_on_select: bool,
    // One flag per input_options::FIELDS entry, in that order.
    choices: [bool; input_options::FIELDS.len()],
}

enum EditIntent {
    Snapshot,
    Density,
    FontFamily,
    SidebarDensity,
}

/// What the settings sheet shows next to its title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SaveStatus {
    Saving,
    Saved,
    Failed { reason: String, retry: bool },
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
    // SH1-157 L1: the host whose terminal view last received the stored
    // Input options and Return (settings_view.rs `sync_input_settings`).
    pub(crate) synced_target: Option<String>,
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
            synced_target: None,
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
        self.edit_values(values, EditIntent::Snapshot);
    }

    // A density gesture resets both fields, even when the pending draft already
    // equals Regular. Polling snapshots must never acquire this explicit intent.
    pub(crate) fn edit_density(&mut self, values: Values) {
        self.edit_values(values, EditIntent::Density);
    }

    // Selecting the displayed default before Loaded still overrides the disk family.
    pub(crate) fn edit_font_family(&mut self, values: Values) {
        self.edit_values(values, EditIntent::FontFamily);
    }

    // Even the displayed Dense default is explicit when picked before loading.
    pub(crate) fn edit_sidebar_density(&mut self, values: Values) {
        self.edit_values(values, EditIntent::SidebarDensity);
    }

    fn edit_values(&mut self, values: Values, intent: EditIntent) {
        if let Err(error) = values.font.validate() {
            self.error = Some(error.to_string());
            return;
        }
        let pending_intent = !self.loaded && !matches!(intent, EditIntent::Snapshot);
        if !self.loaded {
            match intent {
                EditIntent::Density => {
                    self.edited_before_load.compact = true;
                    self.edited_before_load.font_size = true;
                }
                EditIntent::FontFamily => self.edited_before_load.font_family = true,
                EditIntent::Snapshot | EditIntent::SidebarDensity => {}
            }
        }
        if self.draft != values || pending_intent {
            if !self.loaded {
                self.edited_before_load
                    .notifications
                    .track(&self.draft.notifications, &values.notifications);
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
                for (edited, field) in self
                    .edited_before_load
                    .choices
                    .iter_mut()
                    .zip(input_options::FIELDS)
                {
                    *edited |= (matches!(intent, EditIntent::SidebarDensity)
                        && field.key == sidebar_density::SIDEBAR_DENSITY.key)
                        || (field.get)(&self.draft) != (field.get)(&values);
                }
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
                            for (edited, field) in self
                                .edited_before_load
                                .choices
                                .iter()
                                .zip(input_options::FIELDS)
                            {
                                if !edited {
                                    (field.set)(&mut self.draft, (field.get)(&values));
                                }
                            }
                            self.edited_before_load
                                .notifications
                                .merge(&mut self.draft.notifications, values.notifications);
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

    /// The sheet's state line (D7): "Saved" only after the worker reported
    /// the write of the current draft; a failure keeps the draft and offers
    /// Retry unless the store itself could not be read.
    pub(crate) fn save_status(&self) -> SaveStatus {
        match &self.error {
            Some(reason) => SaveStatus::Failed {
                reason: reason.clone(),
                retry: !self.blocked,
            },
            None if self.ready() => SaveStatus::Saved,
            None => SaveStatus::Saving,
        }
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
mod family_tests;

#[cfg(test)]
mod notification_tests;

#[cfg(test)]
mod theme_tests;
