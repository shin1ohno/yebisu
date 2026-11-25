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
/// Controls for the compact sidebar and the first two pane pages.
pub(crate) const COMPACT_MENU: &str = "qa.compact.menu";
pub(crate) const COMPACT_DRAWER_CLOSE: &str = "qa.compact.drawer.close";
pub(crate) const PAGER_PANE_1: &str = "qa.pager.pane.1";
pub(crate) const PAGER_PANE_2: &str = "qa.pager.pane.2";
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
/// SH1-152's QA controls (edit_menu_qa_sh1_152.rs, `YEBISU_SH1_152_CASE`),
/// one per command; Begin keeps its identifier when it reads "Begun". The
/// second E5 scene's controls ("152 Two ...") have their own identifiers.
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_BEGIN: &str = "qa.sh1-152.begin";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_SELECT: &str = "qa.sh1-152.select";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_MUTATE: &str = "qa.sh1-152.mutate";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_REPLAY: &str = "qa.sh1-152.replay";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_FRESH: &str = "qa.sh1-152.fresh";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_END: &str = "qa.sh1-152.end";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_ORDINARY: &str = "qa.sh1-152.ordinary";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_RESTORE: &str = "qa.sh1-152.restore";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_SECOND: &str = "qa.sh1-152.second";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_TWO_BEGIN: &str = "qa.sh1-152.two.begin";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_TWO_SELECT: &str = "qa.sh1-152.two.select";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_TWO_END: &str = "qa.sh1-152.two.end";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_TWO_CLOSE: &str = "qa.sh1-152.two.close";
/// An empty space at the end of SH1-152's strip: no handler, and it stops
/// the touch, so a tap there changes nothing in the app. EditMenuQA presses
/// it after every launch until GPUI counts a press (`touchReady`, as P12
/// does with `P12_EMPTY`): the first touch after a launch can reach nothing.
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the SH1-152 controls are built only in a Simulator build"
    )
)]
pub(crate) const SH1_152_EMPTY: &str = "qa.sh1-152.empty";
/// SH1-153's controls (edit_menu_qa.rs, `YEBISU_EDIT_MENU_QA`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA strip is built only in a Simulator build")
)]
pub(crate) const EDIT_PREPARE: &str = "qa.edit.prepare";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA strip is built only in a Simulator build")
)]
pub(crate) const EDIT_SELECT: &str = "qa.edit.select";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA strip is built only in a Simulator build")
)]
pub(crate) const EDIT_EXECUTE: &str = "qa.edit.execute";
/// P8's "Native next" (native_edit_qa.rs, `YEBISU_NATIVE_QA`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA strip is built only in a Simulator build")
)]
pub(crate) const NATIVE_NEXT: &str = "qa.native.next";
/// SH1-156's "Discard composition" (sh1_156_qa.rs, `YEBISU_SH1_156_QA`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(dead_code, reason = "the QA strip is built only in a Simulator build")
)]
pub(crate) const SH1_156_DISCARD: &str = "qa.sh1-156.discard";
/// The Input page's "Start a selection" and "Copy" choices
/// (settings_sheet/view.rs, `settings_choices`; labels from
/// settings_pages/choices.rs), which SH1-151 sets.
pub(crate) const SETTINGS_START_DOUBLE_TAP: &str = "qa.settings.start-selection.double-tap";
pub(crate) const SETTINGS_START_DRAG: &str = "qa.settings.start-selection.drag";
pub(crate) const SETTINGS_COPY_MANUAL: &str = "qa.settings.copy.manual";
pub(crate) const SETTINGS_COPY_ON_SELECTION: &str = "qa.settings.copy.on-selection";
/// P12's fixture controls (native_menus_qa.rs, `YEBISU_P12_QA`), one per
/// setup step, in the order the panel draws them (UI test redesign S5).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_REMOTE: &str = "qa.p12.remote";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_REARM: &str = "qa.p12.rearm";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_SIDEBAR: &str = "qa.p12.sidebar";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_PALETTE: &str = "qa.p12.palette";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_RELEASE: &str = "qa.p12.release";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_HOST: &str = "qa.p12.host";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_PANE: &str = "qa.p12.pane";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_BOOT: &str = "qa.p12.boot";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_FOCUS: &str = "qa.p12.focus";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_CURRENT: &str = "qa.p12.current";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_CAPABILITY: &str = "qa.p12.capability";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_RESET: &str = "qa.p12.reset";
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_FINISH: &str = "qa.p12.finish";
/// An empty strip at the foot of P12's fixture panel: no handler, and the
/// panel stops the touch, so a tap there changes nothing in the app. P12
/// taps it to close an open OS menu (it replaced a fixed window point).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P12 fixture panel is built only in a Simulator build"
    )
)]
pub(crate) const P12_EMPTY: &str = "qa.p12.empty";
/// The Display page's font size buttons (settings_sheet/view.rs,
/// `settings_fonts`), which P11 presses (UI test redesign S6).
pub(crate) const SETTINGS_FONT_SMALLER: &str = "qa.settings.font.smaller";
pub(crate) const SETTINGS_FONT_LARGER: &str = "qa.settings.font.larger";
/// The Theme page's local palette choices (p15_settings_view.rs), one per
/// `theme_picker::SUPPORTED` palette, which P15 presses.
pub(crate) const SETTINGS_THEME_OWNER_DEFAULT: &str = "qa.settings.theme.owner-default";
pub(crate) const SETTINGS_THEME_HIGH_CONTRAST: &str = "qa.settings.theme.high-contrast";
/// P11's "Probe modal callbacks" (settings_sheet/qa.rs, `YEBISU_P11_QA`
/// without the `test-support` feature).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim", not(feature = "test-support"))),
    allow(dead_code, reason = "the P11 probe is built only in a Simulator build")
)]
pub(crate) const P11_PROBE: &str = "qa.p11.probe";
/// P11's "Observer next" (settings_sheet/observer_qa.rs, `YEBISU_P11_QA`
/// with the `test-support` feature); it keeps its identifier when it reads
/// "Observer matrix collected" or "Observer failed".
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim", feature = "test-support")),
    allow(
        dead_code,
        reason = "the P11 observer is built only in a test-support Simulator build"
    )
)]
pub(crate) const P11_OBSERVER: &str = "qa.p11.observer";
/// P14's "P14 next" (notification_view/qa.rs, `YEBISU_P14_QA`).
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the P14 control is built only in a Simulator build"
    )
)]
pub(crate) const P14_NEXT: &str = "qa.p14.next";
/// The notification toast (notification_view.rs, `render_notification`);
/// a tap on it opens the notification's target.
pub(crate) const NOTIFICATION_TOAST: &str = "qa.notification.toast";
/// The sidebar's host list (shell/sidebar/view.rs, `sidebar-list`), a list
/// that scrolls, and the host rows P15's fixture hosts draw in it (P15
/// presses `qa-beta` to switch hosts). Other hosts have no identifier.
pub(crate) const SIDEBAR_LIST: &str = "qa.sidebar.list";
pub(crate) const SIDEBAR_HOST_QA_ALPHA: &str = "qa.sidebar.host.qa-alpha";
pub(crate) const SIDEBAR_HOST_QA_BETA: &str = "qa.sidebar.host.qa-beta";
/// The sidebar magnifier (shell/filter.rs, `sidebar-search`): it opens the
/// native search panel.
pub(crate) const SIDEBAR_SEARCH: &str = "qa.sidebar.search";
/// The "Clear filter" button of the sidebar's "Filtered by" row
/// (shell/filter.rs, `filter-clear`).
pub(crate) const SIDEBAR_FILTER_CLEAR: &str = "qa.sidebar.filter-clear";
/// Rename in place (issue #143). The field, its clear overlay and its hint
/// are herdr-uikit's UIKit views: yebisu passes these identifiers in
/// `NativeInlineFieldIdentifiers` (shell/rename/native.rs) instead of
/// `qa_ax::mark` (scripts/ios/qa-ax-ids-check.py, NATIVE).
#[cfg_attr(
    not(target_os = "ios"),
    allow(dead_code, reason = "only the iOS SDK adapter names them")
)]
pub(crate) const RENAME_FIELD: &str = "qa.rename.field";
#[cfg_attr(
    not(target_os = "ios"),
    allow(dead_code, reason = "only the iOS SDK adapter names them")
)]
pub(crate) const RENAME_OVERLAY: &str = "qa.rename.overlay";
#[cfg_attr(
    not(target_os = "ios"),
    allow(dead_code, reason = "only the iOS SDK adapter names them")
)]
pub(crate) const RENAME_HINT: &str = "qa.rename.hint";
/// The Rename items of the long-press menus: a space row's
/// (shell/sidebar/long_press.rs), an agent row's, the rail's
/// (shell/rail/menu.rs) and a tab's (shell/tab_row/menu.rs), with the
/// tab menu's Close Tab.
pub(crate) const SIDEBAR_SPACE_RENAME: &str = "qa.sidebar.space-rename";
pub(crate) const SIDEBAR_AGENT_RENAME: &str = "qa.sidebar.agent-rename";
pub(crate) const RAIL_MENU_RENAME: &str = "qa.rail.menu-rename";
pub(crate) const TAB_MENU_RENAME: &str = "qa.tab.menu-rename";
pub(crate) const TAB_MENU_CLOSE: &str = "qa.tab.menu-close";
/// The current space's name in its sidebar row and the current tab's label,
/// where the rename QA (SmokeQA+Rename.swift) long-presses and double-taps.
pub(crate) const SIDEBAR_CURRENT_SPACE: &str = "qa.sidebar.current-space";
pub(crate) const TABS_CURRENT: &str = "qa.tabs.current";
/// The Herdr 1 agent row of the focused pane and the rail's current space,
/// which the rename QA long-presses (S7, S8).
pub(crate) const SIDEBAR_CURRENT_AGENT: &str = "qa.sidebar.current-agent";
pub(crate) const RAIL_CURRENT_SPACE: &str = "qa.rail.current-space";
/// The host row of the saved host named `forward-qa` (scripts/ios/forward-check.py).
pub(crate) const SIDEBAR_HOST_FORWARD_QA: &str = "qa.sidebar.host.forward-qa";
/// The forward sheet (issue 140, shell/forward_sheet/view.rs): its Remote
/// port field, Forward, Close, and the first of the host's listening-port
/// chips. Identifiers are constants, so one names the first chip rather
/// than a port.
pub(crate) const FORWARD_FIELD: &str = "qa.forward.field";
pub(crate) const FORWARD_SUBMIT: &str = "qa.forward.submit";
pub(crate) const FORWARD_CLOSE: &str = "qa.forward.close";
pub(crate) const FORWARD_CHIP: &str = "qa.forward.chip";
/// The sidebar's first forward row (shell/sidebar/forward_row.rs): a tap
/// opens a listening forward's page in Safari.
pub(crate) const FORWARD_ROW: &str = "qa.forward.row";
/// Forward port… in a host row's long-press menu (shell/sidebar/long_press.rs).
pub(crate) const HOST_MENU_FORWARD: &str = "qa.host-menu.forward";
