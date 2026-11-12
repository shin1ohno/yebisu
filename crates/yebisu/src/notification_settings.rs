//! Nonfinal P11 integration adapter against dc2511f; never edits the P11 branch.
//! These controls are mounted on Hosts until the stable sheet contract arrives.
use crate::{app::Smoke, notifications::Route};
use gpui::{AnyElement, AppLifecyclePhase, Context, div, prelude::*};
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
                // Discard delayed foreground effects and stop audio; retain connection policy.
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
    pub(crate) fn render_notification_settings(&self, cx: &Context<Self>) -> AnyElement {
        let policy = &self.notifications.policy;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child("Notifications · connected events only")
            .child(
                div()
                    .id("notification-route")
                    .child(format!("Delivery: {:?} (change)", policy.route))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut policy = this.notifications.policy.clone();
                        policy.route = match policy.route {
                            Route::Off => Route::Toast,
                            Route::Toast => Route::System,
                            Route::System => Route::Off,
                        };
                        this.notifications.set_policy(policy);
                        this.poll_settings(cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("notification-sound")
                    .child(format!("Sound: {} (change)", policy.sound))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut policy = this.notifications.policy.clone();
                        policy.sound = !policy.sound;
                        this.notifications.set_policy(policy);
                        this.poll_settings(cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("notification-volume")
                    .child(format!("Volume: {}% (change)", policy.volume))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut policy = this.notifications.policy.clone();
                        policy.volume = if policy.volume >= 100 {
                            0
                        } else {
                            (policy.volume + 25).min(100)
                        };
                        this.notifications.set_policy(policy);
                        this.poll_settings(cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("notification-mute")
                    .child(format!("Mute: {} (change)", policy.muted))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut policy = this.notifications.policy.clone();
                        policy.muted = !policy.muted;
                        this.notifications.set_policy(policy);
                        this.poll_settings(cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("notification-quiet")
                    .child(format!(
                        "Quiet hours 22:00–08:00: {} (change)",
                        policy.quiet.is_some()
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut policy = this.notifications.policy.clone();
                        policy.quiet = if policy.quiet.is_some() {
                            None
                        } else {
                            Some((22 * 60, 8 * 60))
                        };
                        this.notifications.set_policy(policy);
                        this.poll_settings(cx);
                        cx.notify();
                    })),
            )
            .child(format!(
                "OS permission: {:?}",
                self.notifications.authorization
            ))
            .child(
                div()
                    .id("notification-permission")
                    .child("Request OS notification permission…")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.notifications.permission(true);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
