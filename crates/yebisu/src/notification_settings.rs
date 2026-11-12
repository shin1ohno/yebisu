//! P14 page integrated with the frozen P11 b55f48e9 SettingsHost contract.
//! The single sheet owner still handles focus, persistence, and save fencing.
use crate::{app::Smoke, notifications::Route, settings_sheet::button};
use gpui::{AnyElement, AppLifecyclePhase, Context, div, prelude::*};
#[derive(Clone, Copy)]
enum Preference {
    Route,
    Sound,
    Volume,
    Mute,
    Quiet,
}
impl Smoke {
    pub(crate) fn query_notification_permission(&mut self) {
        self.notifications.permission(false);
    }
    pub(crate) fn notification_lifecycle(&mut self, phase: AppLifecyclePhase) {
        match phase {
            AppLifecyclePhase::Active => {
                self.notifications.active = true;
                self.query_notification_permission();
            }
            AppLifecyclePhase::Inactive | AppLifecyclePhase::Foreground => {
                self.notifications.active = false;
                self.notifications.pause_foreground();
            }
            AppLifecyclePhase::Background | AppLifecyclePhase::Disconnected => {
                self.notifications.active = false;
                self.notifications.invalidate();
                self.terminal.live.notifications.clear();
                for terminal in self.board.hidden.values_mut() {
                    terminal.live.notifications.clear();
                }
            }
        }
    }
    fn edit_notification(&mut self, field: Preference, cx: &mut Context<Self>) {
        if self.settings.is_none() {
            return;
        }
        let mut policy = self.notifications.policy.clone();
        match field {
            Preference::Route => {
                policy.route = match policy.route {
                    Route::Off => Route::Toast,
                    Route::Toast => Route::System,
                    Route::System => Route::Off,
                }
            }
            Preference::Sound => policy.sound = !policy.sound,
            Preference::Volume => {
                policy.volume = if policy.volume >= 100 {
                    0
                } else {
                    (policy.volume + 25).min(100)
                }
            }
            Preference::Mute => policy.muted = !policy.muted,
            Preference::Quiet => {
                policy.quiet = if policy.quiet.is_some() {
                    None
                } else {
                    Some((22 * 60, 8 * 60))
                }
            }
        }
        self.notifications.set_policy(policy);
        self.poll_settings(cx);
        cx.notify();
    }
    pub(crate) fn render_notification_settings(&self, cx: &Context<Self>) -> AnyElement {
        let policy = &self.notifications.policy;
        let rows = [
            (
                "notification-route",
                format!("Delivery: {:?} (change)", policy.route),
                Preference::Route,
            ),
            (
                "notification-sound",
                format!("Sound: {} (change)", policy.sound),
                Preference::Sound,
            ),
            (
                "notification-volume",
                format!("Volume: {}% (change)", policy.volume),
                Preference::Volume,
            ),
            (
                "notification-mute",
                format!("Mute: {} (change)", policy.muted),
                Preference::Mute,
            ),
            (
                "notification-quiet",
                format!(
                    "Quiet hours 22:00–08:00: {} (change)",
                    policy.quiet.is_some()
                ),
                Preference::Quiet,
            ),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child("Notifications · connected events only")
            .children(rows.into_iter().map(|(id, label, field)| {
                button(id, label, self.settings.is_some())
                    .on_click(cx.listener(move |this, _, _, cx| this.edit_notification(field, cx)))
            }))
            .child(format!(
                "OS permission: {:?}",
                self.notifications.authorization
            ))
            .child(
                button(
                    "notification-permission",
                    "Request OS notification permission…",
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.notifications.permission(true);
                    cx.notify();
                })),
            )
            .into_any_element()
    }
}
