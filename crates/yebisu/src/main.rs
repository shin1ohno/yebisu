#[cfg(any(target_os = "ios", test))]
mod diagnostics;
mod error;
#[cfg(any(target_os = "ios", test))]
mod hosts;
#[cfg(any(target_os = "ios", test))]
mod keys;
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
#[allow(dead_code, unused_imports)]
mod upstream;
#[cfg(any(target_os = "ios", test))]
use upstream::{font_weight, osc52, terminal, terminal_painter};
#[cfg(any(target_os = "ios", test))]
mod theme;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod mock_peer;
#[cfg(any(target_os = "ios", test))]
mod terminal_view;

#[cfg(any(target_os = "ios", test))]
mod input_geometry;
#[cfg(any(target_os = "ios", test))]
mod terminal_controls;
#[cfg(any(target_os = "ios", test))]
mod terminal_input;

#[cfg(any(target_os = "ios", test))]
mod reconnect;

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
mod handoff;
#[cfg(any(target_os = "ios", test))]
mod handoff_clipboard;
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
mod native_edit_qa;

#[cfg(any(target_os = "ios", test))]
mod core;
#[cfg(any(target_os = "ios", test))]
mod core_view;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod core_mock;

#[cfg(any(target_os = "ios", test))]
mod settings;
#[cfg(any(target_os = "ios", test))]
mod settings_view;

#[cfg(any(target_os = "ios", test))]
mod i18n;

#[cfg(any(target_os = "ios", test))]
mod settings_spike;

#[cfg(test)]
mod discard_recovery_tests;
