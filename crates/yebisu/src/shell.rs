//! SH1-157 layout shell (Wave 0 skeleton).
//!
//! The integrator owns this file: it holds the contract-frozen shared types
//! and the module declarations. Each submodule belongs to exactly one stream;
//! docs/layout-contracts.md is the ownership table and the signature list.
//! Every slot here renders nothing or keeps the current UI until its stream
//! fills it.

// keep-sorted start
pub(crate) mod add_host;
pub(crate) mod disconnect;
pub(crate) mod edit_bar;
pub(crate) mod entry_move;
pub(crate) mod entry_points;
pub(crate) mod filter;
pub(crate) mod fit;
pub(crate) mod footer;
pub(crate) mod frame;
pub(crate) mod herdr2_frame;
pub(crate) mod host_menus;
pub(crate) mod hosts_sheet;
pub(crate) mod mods;
pub(crate) mod overflow_menu;
pub(crate) mod overlay;
pub(crate) mod qa_strip;
pub(crate) mod rail;
pub(crate) mod rail_badges;
pub(crate) mod shortcuts;
pub(crate) mod sidebar;
pub(crate) mod sidebar_status;
pub(crate) mod single_pane;
pub(crate) mod status_bar;
pub(crate) mod tab_row;
pub(crate) mod toast;
// keep-sorted end

#[cfg(test)]
#[path = "shell/tests.rs"]
mod tests;

/// Which daemon a host connection speaks to (identity key, D8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum HerdrKind {
    #[default]
    Herdr1,
    Herdr2,
}

/// The five chrome states that `chrome_entry_points_cover_required_set`
/// checks (plan v2 5.1 rule 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChromeState {
    pub(crate) kind: HerdrKind,
    pub(crate) connected: bool,
    pub(crate) selecting: bool,
    pub(crate) sheet_open: bool,
    pub(crate) find_available: bool,
    pub(crate) sidebar_hidden: bool,
    /// The width, not the user, holds the sidebar at the rail: Command-B and
    /// the sidebar toggles cannot change anything (Integration 2, L4 review F1).
    pub(crate) sidebar_forced: bool,
}

/// A product entry point (plan v2 5.2, the 19-row table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EntryAction {
    Hosts,
    Settings,
    NewSpace,
    Paste,
    PasteImage,
    Select,
    Copy,
    Reconnect,
    Find,
    NewTab,
    Split,
    Shortcuts,
    ToggleSidebar,
}

/// Where an entry point lives (plan v2 5.2 legend).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Place {
    Toolbar,
    InputBand,
    ReturnBand,
    HandoffBand,
    ActionsBand,
    CoreBand,
    SidebarFooter,
    SidebarHeader,
    TabRow,
    OverflowMenu,
    StatusBar,
    Keyboard,
    LongPress,
    EditBar,
    ShortcutsPopover,
    MenuBar,
    Rail,
    HostsSheet,
    QaStrip,
}

/// One reachable entry point; `reason` explains a disabled one (q4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryPoint {
    pub(crate) action: EntryAction,
    pub(crate) place: Place,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

/// Per-stream view state, one field per owning stream. A stream adds fields
/// only inside its own struct, so `app.rs` never changes for it.
#[derive(Debug, Default)]
pub(crate) struct ShellState {
    pub(crate) frame: frame::FrameState,
    pub(crate) sidebar: sidebar::SidebarState,
    pub(crate) rail: rail::RailState,
    pub(crate) edit_bar: edit_bar::EditBarState,
    pub(crate) shortcuts: shortcuts::ShortcutsState,
    pub(crate) hosts_sheet: hosts_sheet::HostsSheetState,
    pub(crate) single_pane: single_pane::SinglePaneState,
    pub(crate) herdr2: herdr2_frame::Herdr2State,
    pub(crate) fit: fit::FitState,
}
