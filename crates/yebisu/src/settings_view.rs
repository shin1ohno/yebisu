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
                // sh1-156-check.py starts from defaults (Start: Drag), whatever
                // an earlier check in the same Simulator saved.
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_SH1_156_QA").is_some() {
                    root.join("sh1-156-qa")
                } else {
                    root
                };
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_P11_QA").is_some() {
                    root.join("p11-qa")
                } else {
                    root
                };
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_P15_QA").is_some() {
                    root.join("p15-qa")
                } else {
                    root
                };
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                let root = if std::env::var_os("YEBISU_P14_QA").is_some() {
                    root.join("p14-qa")
                } else {
                    root
                };
                crate::settings::SettingsHost::open(root)
            }) {
            Ok(host) => {
                self.settings = Some(host);
                let _ = self.key_provider_preflight("startup");
            }
            Err(error) => self.update_settings_error(Some(error.to_string()), cx),
        }
    }
    pub(crate) fn settings_values(&self) -> Values {
        // Until the selected view received the stored Input options and
        // Return (`sync_input_settings`), its copy may be a new view's
        // defaults: every reader (lifecycle, toolbar, sheet, selection strip)
        // takes the stored values instead (review S2 of 7ffde18).
        let input = self
            .settings
            .as_ref()
            .filter(|host| host.synced_target.as_ref() != Some(&self.terminal.reconnect.target))
            .map_or_else(
                || {
                    let options = &self.terminal.input.options;
                    [
                        options.option_alt,
                        options.command_control,
                        options.caret_anchor,
                        self.terminal.reconnect.automatic,
                    ]
                },
                |host| {
                    let draft = &host.draft;
                    [
                        draft.option_alt,
                        draft.command_control,
                        draft.caret_anchor,
                        draft.reconnect_automatic,
                    ]
                },
            );
        let [
            option_alt,
            command_control,
            caret_anchor,
            reconnect_automatic,
        ] = input;
        Values {
            notifications: self.notifications.policy.clone(),
            theme: self.local_theme,
            font: self.terminal.font.clone(),
            compact: self.terminal.compact,
            terminal_priority: self.actions.terminal_priority,
            board_hidden: self.actions.board_hidden,
            drag_select: self.handoff.drag_select,
            copy_on_select: self.handoff.copy_on_select,
            option_alt,
            command_control,
            caret_anchor,
            reconnect_automatic,
            sidebar_density: self.shell.sidebar.density,
        }
    }
    /// The Input options and Return read by the key encoder, the toolbar
    /// and reconnect (SH1-157 L1) in the selected view: the same fields the
    /// sheet edits. Hidden views take them in `sync_input_settings`.
    fn adopt_input_settings(&mut self, values: &Values) {
        adopt_view_settings(&mut self.terminal, values);
    }
    /// The Input options and Return are app settings, but every terminal
    /// view carries a copy, and views appear with defaults: a host switch
    /// (board.rs `select_host`) swaps in another view, and connecting creates
    /// hidden views (terminal_view.rs `start_host_connection_inner`).
    ///
    /// Hidden views are never edited by the user, so they take the stored
    /// values on every call; each decides its own reconnect on return
    /// (reconnect.rs). The selected view takes them once per host change,
    /// because the toolbar and QA edit it and the next poll reads it back.
    /// Called before the live values are read (`poll_settings`,
    /// `settings_lifecycle`) and before a key is encoded
    /// (`refresh_terminal_input`).
    pub(crate) fn sync_input_settings(&mut self) {
        let Some(host) = &mut self.settings else {
            return;
        };
        let values = host.draft.clone();
        let target = &self.terminal.reconnect.target;
        let selected_synced = host.synced_target.as_ref() == Some(target);
        if !selected_synced {
            host.synced_target = Some(target.clone());
        }
        for terminal in self
            .board
            .terminals_mut(&mut self.terminal)
            .skip(usize::from(selected_synced))
        {
            adopt_view_settings(terminal, &values);
        }
    }
    pub(crate) fn poll_settings(&mut self, cx: &mut gpui::Context<Self>) {
        self.sync_input_settings();
        let values = self.settings_values();
        let Some(host) = &mut self.settings else {
            return;
        };
        host.edit(values);
        if host.keys.poll(std::time::Instant::now()) {
            cx.notify();
        }
        if host.poll() {
            self.observation.record(serde_json::json!({"kind":"settings-loaded", "theme":host.draft.theme.key(), "compact":host.draft.compact, "font_size":host.draft.font.size, "terminal_priority":host.draft.terminal_priority, "drag_select":host.draft.drag_select, "copy_on_select":host.draft.copy_on_select, "option_alt":host.draft.option_alt, "command_control":host.draft.command_control, "caret_anchor":host.draft.caret_anchor, "reconnect_automatic":host.draft.reconnect_automatic}));
            let values = host.draft.clone();
            self.local_theme = host.draft.theme;
            self.terminal.font = host.draft.font.clone();
            self.terminal.compact = host.draft.compact;
            self.notifications.policy = host.draft.notifications.clone();
            self.actions.terminal_priority = host.draft.terminal_priority;
            self.actions.board_hidden = host.draft.board_hidden;
            self.handoff.drag_select = host.draft.drag_select;
            self.handoff.copy_on_select = host.draft.copy_on_select;
            self.shell.sidebar.density = host.draft.sidebar_density;
            self.adopt_input_settings(&values);
            cx.notify();
        }
        let error = self.settings.as_ref().and_then(|host| host.error.clone());
        self.update_settings_error(error, cx);
    }
    fn update_settings_error(&mut self, error: Option<String>, cx: &mut gpui::Context<Self>) {
        if self.settings_error != error {
            self.settings_error = error;
            cx.notify();
        }
    }
    pub(crate) fn settings_lifecycle(&mut self, phase: gpui::AppLifecyclePhase) {
        // reconnect.rs calls this first, before it reads every view's Return.
        self.sync_input_settings();
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if phase == gpui::AppLifecyclePhase::Inactive
            && std::env::var_os("YEBISU_SETTINGS_QA").is_some()
        {
            // An edit immediately before UIKit interrupts the regular save timer.
            self.terminal.compact = true;
            self.actions.terminal_priority = true;
            // SH1-157 L1: the three Input options and Return (Q7, q7).
            let options = &mut self.terminal.input.options;
            options.option_alt = true;
            options.command_control = true;
            options.caret_anchor = true;
            self.terminal.reconnect.automatic = false;
        }
        let values = self.settings_values();
        if let Some(host) = &mut self.settings {
            host.edit(values);
            host.lifecycle(phase);
            self.observation.record(serde_json::json!({"kind":"settings-lifecycle", "phase":format!("{phase:?}"), "ready":host.ready()}));
        }
    }
}

/// The key block's line that says the injected QA key provider is in use;
/// also its QA accessibility label (`qa_ax::ids::KEYS_PROVIDER`).
pub(crate) const KEY_QA_LABEL: &str = "QA injected key provider";

impl Smoke {
    pub(crate) fn key_qa_required(&self) -> bool {
        cfg!(test)
            || (cfg!(all(target_os = "ios", target_abi = "sim"))
                && std::env::vars_os().any(|(name, _)| {
                    let name = name.to_string_lossy();
                    name.starts_with("YEBISU_")
                        && (name.ends_with("_QA") || name.ends_with("_OBSERVE"))
                }))
    }
    pub(crate) fn key_provider_preflight(&self, phase: &'static str) -> bool {
        if !self.key_qa_required() {
            return true;
        }
        let injected = self.settings.as_ref().is_some_and(|h| h.keys.injected());
        let admitted = self.observation.try_record(serde_json::json!({
            "kind":"key-provider-preflight", "phase":phase, "injected":injected
        }));
        if admitted {
            eprintln!("key-provider-preflight injected={injected}");
        }
        injected && admitted
    }
    pub(crate) fn key_qa_label(&self) -> Option<&'static str> {
        (self.key_qa_required() && self.key_provider_preflight("view")).then_some(KEY_QA_LABEL)
    }
    pub(crate) fn copy_device_key(&self, cx: &mut gpui::Context<Self>) -> bool {
        if !self.key_provider_preflight("copy") {
            return false;
        }
        if let Some(line) = self.settings.as_ref().and_then(|h| h.keys.line()) {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(line.to_owned()));
            return true;
        }
        false
    }
    pub(crate) fn confirm_device_key(
        &mut self,
        ticket: crate::settings::key_ops::Ticket,
        visible: bool,
    ) {
        if self.key_provider_preflight("replace")
            && let Some(host) = &mut self.settings
        {
            host.keys.confirm(ticket, visible, false);
        }
    }
    pub(crate) fn read_device_key(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.key_provider_preflight("read") {
            return;
        }
        if let Some(host) = &mut self.settings {
            host.keys.read_or_initialize();
        }
        cx.notify();
    }
    pub(crate) fn select_settings_density(
        &mut self,
        density: crate::settings_pages::fonts::Density,
        cx: &mut gpui::Context<Self>,
    ) {
        let mut values = self.settings_values();
        density.apply(&mut values);
        values.font.size = None;
        if let Some(host) = &mut self.settings {
            host.edit_density(values.clone());
        }
        self.apply_settings(values, cx);
    }

    pub(crate) fn select_settings_font_family(
        &mut self,
        family: String,
        cx: &mut gpui::Context<Self>,
    ) {
        let mut values = self.settings_values();
        values.font.family = family;
        if let Some(host) = &mut self.settings {
            host.edit_font_family(values.clone());
        }
        self.apply_settings(values, cx);
    }

    /// A radio row of the Input or Connection page (SH1-157 L1).
    pub(crate) fn select_settings_choice(
        &mut self,
        choice: crate::settings_pages::choices::Choice,
        value: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        let mut values = self.settings_values();
        choice.set(&mut values, value);
        self.apply_settings(values, cx);
    }

    pub(crate) fn apply_settings(&mut self, values: Values, cx: &mut gpui::Context<Self>) {
        if self.settings.is_none() || values.font.validate().is_err() {
            return;
        }
        self.local_theme = values.theme;
        self.terminal.font = values.font.clone();
        self.terminal.compact = values.compact;
        self.actions.terminal_priority = values.terminal_priority;
        self.actions.board_hidden = values.board_hidden;
        self.handoff.drag_select = values.drag_select;
        self.handoff.copy_on_select = values.copy_on_select;
        self.shell.sidebar.density = values.sidebar_density;
        self.adopt_input_settings(&values);
        if let Some(host) = &mut self.settings {
            host.edit(values);
            host.flush();
        }
        cx.notify();
    }
}

fn adopt_view_settings(terminal: &mut crate::terminal_view::TerminalView, values: &Values) {
    let options = &mut terminal.input.options;
    options.option_alt = values.option_alt;
    options.command_control = values.command_control;
    options.caret_anchor = values.caret_anchor;
    terminal.reconnect.automatic = values.reconnect_automatic;
}
