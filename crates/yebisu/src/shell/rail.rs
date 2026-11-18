//! L4: the 72 pt rail. Owner: L4.
//!
//! The sidebar has two modes. `Full` is 300 pt, `Rail` is 72 pt (plan v2 7.9,
//! Q2). The user picks one with Command-B or the sidebar button; the window
//! can force `Rail` when 300 plus one pane at its minimum does not fit.
//!
//! The user's choice is the existing `board_hidden` flag, which is already
//! saved with the settings: hidden means collapsed to the rail. That keeps one
//! source of truth for `Command::ToggleSidebar`, the settings file and the
//! menu bar, and the legacy board band keeps its behaviour until the frame
//! hosts the sidebar.
use super::{ChromeState, EntryAction, EntryPoint, HerdrKind, Place, rail_badges::RailBadges};
use crate::{
    app::Smoke,
    layout::{RULE, SIDEBAR_WIDTH, TERMINAL_MIN_WIDTH},
};
use gpui::{AnyElement, Context};

mod model;
mod view;

pub(crate) use model::{RailHost, RailItem, RailItemKind, RailSpace, RailTarget, rail_items};

/// The rail's own state (the filter text lives here too, see `filter.rs`).
#[derive(Debug, Default)]
pub(crate) struct RailState {
    /// The badge mode last written to the observation log; a record is
    /// written only when it changes.
    pub(super) recorded_badges: Option<RailBadges>,
    /// The measured badge widths are logged once.
    pub(super) widths_recorded: bool,
    /// The sidebar filter text (`filter.rs`).
    pub(super) filter: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SidebarMode {
    #[default]
    Full,
    Rail,
}

/// The narrowest window that still holds the full sidebar and one pane at its
/// minimum width (Q2). The same sum `layout::compute` uses, after M2 has been
/// withdrawn.
pub(crate) const FULL_MIN_WINDOW: f32 = SIDEBAR_WIDTH + RULE + TERMINAL_MIN_WIDTH;

/// The mode for a window width and the user's choice (Q2: collapse last).
/// `window_width` is the usable width: the window minus the side safe areas.
/// M2 (the right dock) gives way before this collapses, so M2 plays no part
/// here.
pub(crate) fn sidebar_mode(window_width: f32, user: SidebarMode) -> SidebarMode {
    if user == SidebarMode::Rail || window_width < FULL_MIN_WINDOW {
        SidebarMode::Rail
    } else {
        SidebarMode::Full
    }
}

/// The user's choice behind the saved `board_hidden` flag.
pub(crate) fn user_mode(sidebar_hidden: bool) -> SidebarMode {
    if sidebar_hidden {
        SidebarMode::Rail
    } else {
        SidebarMode::Full
    }
}

/// Why New space is disabled, or `None` when it works (q4).
pub(crate) fn new_space_unavailable(kind: HerdrKind, connected: bool) -> Option<&'static str> {
    match (kind, connected) {
        (HerdrKind::Herdr2, _) => Some("Not available on Herdr 2"),
        (HerdrKind::Herdr1, false) => Some("Not connected"),
        (HerdrKind::Herdr1, true) => None,
    }
}

/// The entry points the rail itself offers. They exist while the rail is what
/// is on screen, that is while `state.sidebar_hidden` is set (the frame sets
/// it from the effective mode); Hosts, Settings and New space then have no
/// other place in the sidebar column.
pub(crate) fn rail_entry_points(state: &ChromeState) -> Vec<EntryPoint> {
    if !state.sidebar_hidden {
        return Vec::new();
    }
    let new_space = new_space_unavailable(state.kind, state.connected);
    let entry = |action, enabled, reason| EntryPoint {
        action,
        place: Place::Rail,
        enabled,
        reason,
    };
    vec![
        entry(EntryAction::Hosts, true, None),
        entry(EntryAction::Settings, true, None),
        entry(EntryAction::NewSpace, new_space.is_none(), new_space),
        entry(
            EntryAction::ToggleSidebar,
            toggle_unavailable(state).is_none(),
            toggle_unavailable(state),
        ),
    ]
}

/// Sizes in points. Nothing in the rail is smaller than 11 pt (D10) and the
/// buttons grow with the text scale (S-11); a scale below 1 changes nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RailMetrics {
    pub(crate) label_pt: f32,
    pub(crate) badge_pt: f32,
    pub(crate) item_height: f32,
    pub(crate) badge_height: f32,
}

pub(crate) const LABEL_PT: f32 = 11.0;
/// Height of a host or space button at the default text scale.
pub(crate) const ITEM_BASE_HEIGHT: f32 = 56.0;
/// Padding above the list and below the footer; the gap between buttons.
pub(crate) const EDGE_PAD: f32 = 8.0;
pub(crate) const ITEM_GAP: f32 = 6.0;
pub(crate) const FOOTER_BUTTON_HEIGHT: f32 = 44.0;
/// New space, Hosts, Settings and the expand button.
pub(crate) const FOOTER_BUTTONS: usize = 4;

pub(crate) fn rail_metrics(text_scale: f32) -> RailMetrics {
    let scale = if text_scale.is_finite() {
        text_scale.max(1.0)
    } else {
        1.0
    };
    RailMetrics {
        label_pt: LABEL_PT * scale,
        badge_pt: super::rail_badges::BADGE_TEXT_PT * scale,
        item_height: super::sidebar::row_height(ITEM_BASE_HEIGHT, scale).ceil(),
        badge_height: (super::rail_badges::BADGE_HEIGHT * scale).ceil(),
    }
}

/// The rail's vertical split: a fixed header, a scrolling list, a fixed
/// footer (plan v2 7.9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RailSections {
    pub(crate) header: f32,
    pub(crate) list: f32,
    pub(crate) footer: f32,
    pub(crate) list_scrolls: bool,
}

pub(crate) const RAIL_HEADER_HEIGHT: f32 = EDGE_PAD;
pub(crate) const RAIL_FOOTER_HEIGHT: f32 =
    FOOTER_BUTTONS as f32 * (FOOTER_BUTTON_HEIGHT + ITEM_GAP) + EDGE_PAD;

/// Splits `available` height; the list takes what the fixed parts leave and
/// scrolls when its `content` is taller than that.
pub(crate) fn rail_sections(available: f32, content: f32) -> RailSections {
    let list = (available - RAIL_HEADER_HEIGHT - RAIL_FOOTER_HEIGHT).max(0.0);
    RailSections {
        header: RAIL_HEADER_HEIGHT,
        list,
        footer: RAIL_FOOTER_HEIGHT,
        list_scrolls: content > list,
    }
}

/// Why the rail's expand button is off: the user wants 300 but the window is
/// too narrow for it, so expanding would change nothing (S1 of review 1).
pub(crate) fn expand_unavailable(user: SidebarMode, collapsed: bool) -> Option<&'static str> {
    (user == SidebarMode::Full && collapsed).then_some(WINDOW_TOO_NARROW)
}

/// Why every sidebar toggle is off while the width forces the rail.
pub(crate) const WINDOW_TOO_NARROW: &str = "Window too narrow";

/// Whether a sidebar toggle can work, and why not (Integration 2, L4 review F1).
pub(crate) fn toggle_unavailable(state: &ChromeState) -> Option<&'static str> {
    state.sidebar_forced.then_some(WINDOW_TOO_NARROW)
}

impl Smoke {
    /// The window, not the user, holds the sidebar at 72: the user's choice is
    /// 300 and the last frame was collapsed anyway.
    pub(crate) fn sidebar_forced_rail(&self) -> bool {
        expand_unavailable(self.sidebar_user_mode(), self.shell.frame.sidebar_collapsed).is_some()
    }

    /// `Command::ToggleSidebar` (stays a local command, PR 44's oracle).
    /// Command-B and the header button switch between 300 and the rail by
    /// flipping the saved `board_hidden` choice. While the window forces the
    /// rail nothing can change, so the saved choice is left alone.
    pub(crate) fn toggle_sidebar(&mut self) {
        if self.sidebar_forced_rail() {
            return;
        }
        self.actions.board_hidden ^= true;
    }

    /// The rail's expand button: sets the choice to 300, never flips it.
    /// Returns false while the window forces the rail.
    pub(crate) fn expand_sidebar(&mut self) -> bool {
        if self.sidebar_forced_rail() {
            return false;
        }
        self.actions.board_hidden = false;
        true
    }

    /// The user's saved choice of mode; the window may still force the rail.
    pub(crate) fn sidebar_user_mode(&self) -> SidebarMode {
        user_mode(self.actions.board_hidden)
    }

    /// The rail column. The frame calls it only when the sidebar is collapsed
    /// (`sidebar_mode(..) == Rail`); it draws the column unconditionally.
    pub(crate) fn render_rail(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        Some(self.rail_column(cx))
    }
}

#[cfg(test)]
#[path = "rail_tests.rs"]
mod tests;
