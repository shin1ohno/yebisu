//! L1: the toolbar toggles write the same store as the settings sheet
//! (S-6). Owner: L1.
use crate::app::Smoke;
use gpui::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarSetting {
    Density,
    ReturnPolicy,
    InputOptions,
}

impl Smoke {
    /// Called by the legacy toolbar after it changed `setting` in memory:
    /// the sheet's draft takes the new value at once and the store writes
    /// it, the same path as a change made in the sheet. Every setting is
    /// read from the live state, so `setting` names the trigger only.
    pub(crate) fn toolbar_setting_changed(
        &mut self,
        setting: ToolbarSetting,
        cx: &mut Context<Self>,
    ) {
        // The legacy Return toggle (terminal_view.rs) flips the selected
        // view before calling here; if that view had not received the stored
        // values yet, keep the flip and give it the stored rest.
        let flipped_return = self.terminal.reconnect.automatic;
        self.sync_input_settings();
        if setting == ToolbarSetting::ReturnPolicy {
            self.terminal.reconnect.automatic = flipped_return;
        }
        let values = self.settings_values();
        let Some(host) = &mut self.settings else {
            return;
        };
        if setting == ToolbarSetting::Density {
            host.edit_density(values);
        } else {
            host.edit(values);
        }
        host.flush();
        cx.notify();
    }
}
