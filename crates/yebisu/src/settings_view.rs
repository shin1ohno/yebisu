//! UIKit/GPUI adapter. Settings flush precedes reconnect's suspension early return.
use crate::{app::Smoke, settings::Values};

impl Smoke {
    #[cfg(target_os = "ios")]
    pub(crate) fn start_settings(&mut self, cx: &mut gpui::Context<Self>) {
        match sheprd_client::local_path::state_dir()
            .ok_or_else(|| crate::Error::Settings("state directory unavailable".into()))
            .and_then(|root| {
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_SETTINGS_QA").is_some() {
                    root.join("p10-qa")
                } else {
                    root
                };
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_SELECTION_SETTINGS_QA").is_some() {
                    root.join("sh1-151-qa")
                } else {
                    root
                };
                crate::settings::SettingsHost::open(root)
            }) {
            Ok(host) => self.settings = Some(host),
            Err(error) => self.update_settings_error(Some(error.to_string()), cx),
        }
    }
    pub(crate) fn settings_values(&self) -> Values {
        Values {
            font: self.terminal.font.clone(),
            compact: self.terminal.compact,
            terminal_priority: self.actions.terminal_priority,
            board_hidden: self.actions.board_hidden,
            drag_select: self.handoff.drag_select,
            copy_on_select: self.handoff.copy_on_select,
        }
    }
    pub(crate) fn poll_settings(&mut self, cx: &mut gpui::Context<Self>) {
        let values = self.settings_values();
        let Some(host) = &mut self.settings else {
            return;
        };
        host.edit(values);
        if host.poll() {
            self.observation.record(serde_json::json!({"kind":"settings-loaded", "compact":host.draft.compact, "terminal_priority":host.draft.terminal_priority, "drag_select":host.draft.drag_select, "copy_on_select":host.draft.copy_on_select}));
            self.terminal.font = host.draft.font.clone();
            self.terminal.compact = host.draft.compact;
            self.actions.terminal_priority = host.draft.terminal_priority;
            self.actions.board_hidden = host.draft.board_hidden;
            self.handoff.drag_select = host.draft.drag_select;
            self.handoff.copy_on_select = host.draft.copy_on_select;
            cx.notify();
        }
        let error = host.error.clone();
        self.update_settings_error(error, cx);
    }
    fn update_settings_error(&mut self, error: Option<String>, cx: &mut gpui::Context<Self>) {
        if self.settings_error != error {
            self.settings_error = error;
            cx.notify();
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
