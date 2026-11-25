#[cfg(any(target_os = "ios", test))]
mod diagnostics;
mod error;
#[cfg(any(target_os = "ios", test))]
mod hosts;
#[cfg(any(target_os = "ios", test))]
mod keys;
mod layout;
#[cfg(any(target_os = "ios", test))]
mod setup;
pub use error::{Error, Result};
#[cfg(any(target_os = "ios", test))]
mod app;
#[cfg(any(target_os = "ios", test))]
mod app_setup;
#[cfg(any(target_os = "ios", test))]
mod input;

fn main() {
    #[cfg(target_os = "ios")]
    app::run();
}

#[cfg(any(target_os = "ios", test))]
mod observation;

#[cfg(any(target_os = "ios", test))]
mod link;
#[cfg(any(target_os = "ios", test))]
mod live;
#[cfg(any(target_os = "ios", test))]
mod notification_settings;
#[cfg(any(target_os = "ios", test))]
mod notification_view;
#[cfg(any(target_os = "ios", test))]
mod notifications;
#[cfg(any(target_os = "ios", test))]
#[allow(dead_code, unused_imports)]
mod upstream;
#[cfg(any(target_os = "ios", test))]
use upstream::{font_weight, osc52, terminal, terminal_painter};
#[cfg(any(target_os = "ios", test))]
mod theme;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod animation_qa;
#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod mock_peer;
#[cfg(any(target_os = "ios", test))]
mod terminal_links;
#[cfg(any(target_os = "ios", test))]
mod terminal_view;
#[cfg(any(target_os = "ios", test))]
mod text_gate;

#[cfg(any(target_os = "ios", test))]
mod input_geometry;
#[cfg(any(target_os = "ios", test))]
mod input_indicator;
#[cfg(any(target_os = "ios", test))]
mod terminal_controls;
#[cfg(any(target_os = "ios", test))]
mod terminal_input;

#[cfg(any(target_os = "ios", test))]
mod reconnect;

#[cfg(any(target_os = "ios", test))]
mod launch_restore;

#[cfg(any(target_os = "ios", test))]
#[allow(
    dead_code,
    reason = "the forward sheet and the sidebar rows (issue 140's UI) call the entry points, and the iOS test build has no SSH scan caller until then"
)]
mod forwards;

#[cfg(any(target_os = "ios", test))]
mod keep_awake;

#[cfg(any(target_os = "ios", test))]
mod board;

#[cfg(any(target_os = "ios", test))]
mod action_view;
#[cfg(any(target_os = "ios", test))]
mod actions;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod mock_commands;

#[cfg(test)]
mod review_r5_tests;

#[cfg(any(target_os = "ios", test))]
mod core_paste_view;
#[cfg(any(target_os = "ios", test))]
mod handoff;
#[cfg(any(target_os = "ios", test))]
mod handoff_clipboard;
#[cfg(any(target_os = "ios", test))]
mod handoff_ticket;
#[cfg(any(target_os = "ios", test))]
mod handoff_view;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod handoff_qa;

#[cfg(test)]
mod handoff_cjk_tests;

#[cfg(test)]
mod handoff_review_tests;

#[cfg(test)]
mod handoff_resume_tests;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod edit_menu_qa;
#[cfg(all(target_os = "ios", target_abi = "sim", not(feature = "test-support")))]
mod native_edit_qa;
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
mod native_observer_qa;
#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod sh1_156_qa;

#[cfg(any(target_os = "ios", test))]
mod core;
#[cfg(any(target_os = "ios", test))]
mod core_view;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod core_mock;

#[cfg(any(target_os = "ios", test))]
mod settings;
#[cfg(any(target_os = "ios", test))]
mod settings_pages;
#[cfg(any(target_os = "ios", test))]
mod settings_sheet;
#[cfg(any(target_os = "ios", test))]
mod settings_view;

#[cfg(any(target_os = "ios", test))]
mod i18n;

#[cfg(any(target_os = "ios", test))]
mod settings_spike;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod mock_notifications;

#[cfg(any(target_os = "ios", test))]
mod find;

#[cfg(any(target_os = "ios", test))]
mod find_view;

#[cfg(any(target_os = "ios", test))]
mod edit_menu;
#[cfg(any(target_os = "ios", test))]
mod edit_menu_gestures;
#[cfg(test)]
mod edit_menu_tests;
#[cfg(any(target_os = "ios", test))]
mod front_layer;
#[cfg(any(target_os = "ios", test))]
mod native_menus;
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
mod p11_observer;
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
mod qa_stderr;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod edit_menu_qa_native;
#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod edit_menu_qa_scenes;
#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod edit_menu_qa_sh1_152;
#[cfg(any(target_os = "ios", test))]
#[path = "host_settings/availability.rs"]
mod host_settings_availability;
#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod native_menus_qa;
#[cfg(any(target_os = "ios", test))]
mod p15_settings_view;
#[cfg(any(target_os = "ios", test))]
mod qa_ax;
#[cfg(any(target_os = "ios", test))]
mod theme_picker;

#[cfg(test)]
mod discard_recovery_tests;

#[cfg(test)]
mod switch_restore_tests;

#[cfg(test)]
mod discard_connection_tests;

#[cfg(test)]
mod direct_text_recovery_tests;

#[cfg(any(target_os = "ios", test))]
mod input_retirement;
#[cfg(target_os = "ios")]
mod native_retirement;

#[cfg(test)]
mod sh1_156_tap_tests;

// SH1-157 layout skeleton (docs/layout-contracts.md). Wave 0 slots are
// declared before their streams call them; the integrator removes this
// allow at the end of Wave 3.
#[cfg(any(target_os = "ios", test))]
#[allow(dead_code)]
mod shell;
