//! The SettingsWindow-to-owner cut: the real SettingsHost still owns all saves.
use crate::{app::Smoke, settings::Values};
use gpui::{App, WeakEntity};

#[derive(Clone, Copy)]
pub(super) enum Field {
    Compact,
    TerminalPriority,
    BoardHidden,
}
#[derive(Clone)]
pub(super) struct Draft {
    pub source: WeakEntity<Smoke>,
}
impl Draft {
    pub fn values(&self, cx: &App) -> Values {
        self.source
            .upgrade()
            .and_then(|s| s.read(cx).settings.as_ref().map(|h| h.draft.clone()))
            .unwrap_or_default()
    }
    pub fn set(&self, field: Field, value: bool, cx: &mut App) {
        let _ = self.source.update(cx, |source, cx| {
            match field {
                Field::Compact => source.terminal.compact = value,
                Field::TerminalPriority => source.actions.terminal_priority = value,
                Field::BoardHidden => source.actions.board_hidden = value,
            }
            source.poll_settings(cx);
        });
    }
    pub fn ready(&self, cx: &App) -> bool {
        self.source.upgrade().is_some_and(|s| {
            s.read(cx)
                .settings
                .as_ref()
                .is_some_and(crate::settings::SettingsHost::ready)
        })
    }
    pub fn flush(&self, cx: &mut App) {
        let _ = self.source.update(cx, |source, cx| {
            source.poll_settings(cx);
            if let Some(host) = &mut source.settings {
                host.flush();
            }
        });
    }
    pub fn confirm_regeneration(&self, cx: &mut App) {
        let _ = self.source.update(cx, |source, _| {
            if let Some(host) = &mut source.settings {
                host.confirm_regeneration();
            }
        });
    }
    pub fn error(&self, cx: &App) -> Option<String> {
        self.source
            .upgrade()
            .and_then(|s| s.read(cx).settings_error.clone())
    }
}
// The spike never instantiates unsupported page backends merely to hide them.
pub(super) fn available(id: &str) -> bool {
    matches!(id, "fonts-density" | "keyboard-priority" | "sidebar-hidden")
}
