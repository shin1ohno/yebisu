//! Container-local draft ownership. Disk work is serialized off the UI thread.
mod store;
mod worker;

use crate::Result;
use std::{path::PathBuf, sync::mpsc};
use worker::{Completion, Request};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Values {
    pub compact: bool,
    pub terminal_priority: bool,
    pub board_hidden: bool,
}

pub(crate) struct SettingsHost {
    pub draft: Values,
    pub error: Option<String>,
    revision: u64,
    saved: u64,
    loaded: bool,
    edited_before_load: Values,
    blocked: bool,
    write_failed: bool,
    in_flight: Option<u64>,
    sender: mpsc::SyncSender<Request>,
    receiver: mpsc::Receiver<Completion>,
    #[cfg(target_os = "ios")]
    background: Option<herdr_uikit::BackgroundTask>,
    // P11 owns the confirmation UI; no regeneration is executed in P10.
    regeneration_confirmed: bool,
}
impl SettingsHost {
    pub fn open(root: PathBuf) -> Result<Self> {
        let (sender, receiver) = worker::start(root)?;
        Ok(Self {
            draft: Values::default(),
            error: None,
            revision: 0,
            saved: 0,
            loaded: false,
            edited_before_load: Values::default(),
            blocked: false,
            write_failed: false,
            in_flight: None,
            sender,
            receiver,
            #[cfg(target_os = "ios")]
            background: None,
            regeneration_confirmed: false,
        })
    }

    pub fn edit(&mut self, values: Values) {
        if self.draft != values {
            if !self.loaded {
                self.edited_before_load.compact |= self.draft.compact != values.compact;
                self.edited_before_load.terminal_priority |=
                    self.draft.terminal_priority != values.terminal_priority;
                self.edited_before_load.board_hidden |=
                    self.draft.board_hidden != values.board_hidden;
            }
            self.write_failed = false;
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
                            if !self.edited_before_load.compact {
                                self.draft.compact = values.compact;
                            }
                            if !self.edited_before_load.terminal_priority {
                                self.draft.terminal_priority = values.terminal_priority;
                            }
                            if !self.edited_before_load.board_hidden {
                                self.draft.board_hidden = values.board_hidden;
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
        #[cfg(target_os = "ios")]
        if self.ready() || self.error.is_some() {
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
                self.write_failed = true;
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
        match self.sender.try_send(Request {
            revision: self.revision,
            values: self.draft.clone(),
        }) {
            Ok(()) => self.in_flight = Some(self.revision),
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
            #[cfg(target_os = "ios")]
            if !self.ready() && !self.blocked && self.background.is_none() {
                self.background = herdr_uikit::BackgroundTask::begin("Saving yebisu settings");
            }
            self.flush();
        }
        if phase == gpui::AppLifecyclePhase::Active {
            self.write_failed = false;
            self.flush();
        }
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
