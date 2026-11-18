//! Every QA accessibility identifier, in one place.
//!
//! XCTest finds a marked control by one of these strings. Each constant
//! here (except `STATE`, which the app root publishes) must be passed to
//! `qa_ax::mark` somewhere, and every identifier a Swift test names must be
//! defined here: scripts/ios/qa-ax-ids-check.py checks both directions.
//! Names are `qa.<screen or area>.<control>`.

/// The state element; its value is one line of JSON (`layer::state_json`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "only the Simulator's UIKit layer names it")
)]
pub(crate) const STATE: &str = "qa.state";
/// The P0 smoke screen's "Tap / type" control (the S1 self-test presses it).
pub(crate) const P0_TAP: &str = "qa.p0.tap";
/// The key block's "QA injected key provider" line (shell/hosts_sheet/view.rs,
/// `key_block`): drawn only after the injected key provider passed its view
/// preflight, so finding it is the P2 provider check.
pub(crate) const KEYS_PROVIDER: &str = "qa.keys.provider";
/// The key block's "Read or initialize device key" button.
pub(crate) const KEYS_READ: &str = "qa.keys.read";
/// The key block's "Copy authorized_keys line" button.
pub(crate) const KEYS_COPY: &str = "qa.keys.copy";
