//! UIKit/GPUI adapter. Settings flush precedes reconnect's suspension early return.
use crate::{app::Smoke, settings::Values};

impl Smoke {
    #[cfg(target_os = "ios")]
    pub(crate) fn start_settings(&mut self) {
        match sheprd_client::local_path::state_dir()
            .ok_or_else(|| crate::Error::Settings("state directory unavailable".into()))
            .and_then(|root| {
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_SETTINGS_QA").is_some() {
                    root.join("p10-qa")
                } else {
                    root
                };
                crate::settings::SettingsHost::open(root)
            }) {
            Ok(host) => self.settings = Some(host),
            Err(error) => self.setup.status = error.to_string(),
        }
    }
    pub(crate) fn settings_values(&self) -> Values {
        Values {
            compact: self.terminal.compact,
            terminal_priority: self.actions.terminal_priority,
            board_hidden: self.actions.board_hidden,
        }
    }
    pub(crate) fn poll_settings(&mut self) {
        let values = self.settings_values();
        let Some(host) = &mut self.settings else {
            return;
        };
        host.edit(values);
        if host.poll() {
            self.observation.record(serde_json::json!({"kind":"settings-loaded", "compact":host.draft.compact, "terminal_priority":host.draft.terminal_priority}));
            self.terminal.compact = host.draft.compact;
            self.actions.terminal_priority = host.draft.terminal_priority;
            self.actions.board_hidden = host.draft.board_hidden;
        }
        if let Some(error) = &host.error {
            self.setup.status = format!("Settings: {error}");
        }
    }
    pub(crate) fn settings_lifecycle(&mut self, phase: gpui::AppLifecyclePhase) {
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if phase == gpui::AppLifecyclePhase::Inactive
            && std::env::var_os("YEBISU_SETTINGS_QA").is_some()
        {
            // An edit immediately before UIKit interrupts the regular save timer.
            self.terminal.compact = true;
            self.actions.terminal_priority = true;
        }
        let values = self.settings_values();
        if let Some(host) = &mut self.settings {
            host.edit(values);
            host.lifecycle(phase);
            self.observation.record(serde_json::json!({"kind":"settings-lifecycle", "phase":format!("{phase:?}"), "ready":host.ready()}));
        }
    }
}
