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
/// The tab row's "…" button that opens and closes the overflow menu
/// (shell/tab_row.rs, `shell-more`).
pub(crate) const TABS_MORE: &str = "qa.tabs.more";
/// The overflow menu's items (shell/overflow_menu.rs, `OverflowItem`), one
/// per item in `OverflowItem` order.
pub(crate) const MENU_PASTE: &str = "qa.menu.paste";
pub(crate) const MENU_PASTE_IMAGE: &str = "qa.menu.paste-image";
pub(crate) const MENU_SELECT_TEXT: &str = "qa.menu.select-text";
pub(crate) const MENU_COPY: &str = "qa.menu.copy";
pub(crate) const MENU_FIND: &str = "qa.menu.find";
pub(crate) const MENU_COMMAND_PALETTE: &str = "qa.menu.command-palette";
pub(crate) const MENU_RECONNECT: &str = "qa.menu.reconnect";
pub(crate) const MENU_CLOSE_TAB: &str = "qa.menu.close-tab";
pub(crate) const MENU_DETACH: &str = "qa.menu.detach";
pub(crate) const MENU_NEW_TAB: &str = "qa.menu.new-tab";
pub(crate) const MENU_SPLIT: &str = "qa.menu.split";
pub(crate) const MENU_PANE_1: &str = "qa.menu.pane-1";
pub(crate) const MENU_PANE_2: &str = "qa.menu.pane-2";
pub(crate) const MENU_SPLIT_DIRECTION: &str = "qa.menu.split-direction";
/// The QA band's input controls (terminal_controls.rs, `YEBISU_INPUT_QA`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA band is built only in a Simulator build")
)]
pub(crate) const BAND_MARK: &str = "qa.band.mark";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA band is built only in a Simulator build")
)]
pub(crate) const BAND_COMMIT: &str = "qa.band.commit";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA band is built only in a Simulator build")
)]
pub(crate) const BAND_DELETE: &str = "qa.band.delete";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA band is built only in a Simulator build")
)]
pub(crate) const BAND_CANCEL: &str = "qa.band.cancel";
/// The QA band's image controls (handoff_qa.rs, `YEBISU_HANDOFF_QA`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA band is built only in a Simulator build")
)]
pub(crate) const BAND_IMAGE: &str = "qa.band.image";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA band is built only in a Simulator build")
)]
pub(crate) const BAND_IMAGE_ONLY: &str = "qa.band.image-only";
/// The status bar's Reconnect chip (shell/status_bar.rs), shown while
/// disconnected.
pub(crate) const STATUS_RECONNECT: &str = "qa.status.reconnect";
/// The sidebar footer's Settings (shell/footer.rs) and the rail's Settings
/// (shell/rail/view.rs), which opens the same sheet when the sidebar is a rail.
pub(crate) const FOOTER_SETTINGS: &str = "qa.footer.settings";
pub(crate) const RAIL_SETTINGS: &str = "qa.rail.settings";
/// The settings sheet's Close (settings_sheet/view.rs).
pub(crate) const SETTINGS_CLOSE: &str = "qa.settings.close";
/// The settings sheet's section buttons, one per `Section`
/// (settings_pages/sections.rs), in canvas order.
pub(crate) const SETTINGS_PAGE_DISPLAY: &str = "qa.settings.page.display";
pub(crate) const SETTINGS_PAGE_THEME: &str = "qa.settings.page.theme";
pub(crate) const SETTINGS_PAGE_INPUT: &str = "qa.settings.page.input";
pub(crate) const SETTINGS_PAGE_SHORTCUTS: &str = "qa.settings.page.shortcuts";
pub(crate) const SETTINGS_PAGE_CONNECTION: &str = "qa.settings.page.connection";
pub(crate) const SETTINGS_PAGE_NOTIFICATIONS: &str = "qa.settings.page.notifications";
pub(crate) const SETTINGS_PAGE_HOSTS_AND_KEYS: &str = "qa.settings.page.hosts-and-keys";
pub(crate) const SETTINGS_PAGE_ABOUT: &str = "qa.settings.page.about";
/// The Display page's density buttons (settings_sheet/view.rs,
/// `settings_fonts`).
pub(crate) const SETTINGS_DENSITY_REGULAR: &str = "qa.settings.density.regular";
pub(crate) const SETTINGS_DENSITY_COMPACT: &str = "qa.settings.density.compact";
/// The terminal's input area (terminal_view.rs, `terminal-input`): a tap
/// there gives the terminal the keyboard.
pub(crate) const TERMINAL_INPUT: &str = "qa.terminal.input";
