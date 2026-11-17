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
    /// Called by the legacy toolbar after it changed `setting` in memory.
    /// Wave 0: no-op (nothing is persisted from the toolbar yet).
    pub(crate) fn toolbar_setting_changed(
        &mut self,
        _setting: ToolbarSetting,
        _cx: &mut Context<Self>,
    ) {
    }
}
