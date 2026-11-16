//! The search close hands input back while the startup settings store is loaded.
//!
//! On iOS `start_settings` stores the SettingsHost at launch, so `settings` is
//! Some for the app's lifetime. Only the settings sheet (`settings_modal`)
//! owns input; the loaded store must not block the retirement ticket.
#![allow(clippy::unwrap_used)]
use super::available;
use crate::{app::Smoke, settings::SettingsHost};
use gpui::TestAppContext;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[gpui::test]
fn close_hands_input_back_while_the_startup_settings_store_is_loaded(cx: &mut TestAppContext) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/p13-tests")
        .join(format!("{}-retirement-owner", std::process::id()));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, _receipts) = crate::find::peer::connect().unwrap();
    view.update(cx, |this, _| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap());
        this.select_host("p13-owner");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        available(&this.terminal)
    }) {
        assert!(Instant::now() < deadline, "P13 peer startup");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        window.activate_window();
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.open_find(window, cx);
            this.close_find(window, cx);
            assert!(this.find.is_none() && this.terminal_focus.is_focused(window));
            assert!(this.find_handoff.paused, "close lost the native fence");
            assert!(this.settings.is_some() && !this.settings_modal);

            // The settings sheet still owns input: no ticket while it is shown.
            this.settings_modal = true;
            assert!(
                this.begin_find_retirement(40).is_none(),
                "settings sheet lost input ownership"
            );
            this.settings_modal = false;

            let ticket = this
                .begin_find_retirement(40)
                .unwrap_or_else(|| panic!("loaded settings store blocked the input owner"));
            assert!(this.complete_find_retirement(&ticket, 40, 41, window));
            assert!(!this.find_handoff.paused, "fence survived the retirement");
            assert!(this.find_keyboard_status().is_none());
        })
    });
    let _ = std::fs::remove_dir_all(root);
}
