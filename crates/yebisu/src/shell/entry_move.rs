//! L2b: retires the legacy bands. Owner: L2b.
//!
//! Plan v2 7.6: the two toolbar rows, the Input options band, the Return
//! band, the handoff band and the tab-and-button band are gone, and the six
//! QA bands moved into the QA strip. Each one left in the same change that
//! gave its entry points a new place (plan v2 5.1 rule 1):
//!
//! | Band | New place |
//! |---|---|
//! | toolbar: Hosts, Settings | sidebar footer (`footer.rs`) |
//! | toolbar: Reconnect | status bar and the ... menu (L2a) |
//! | toolbar: Standard / Compact, Input options | Settings › Display, Input (L1) |
//! | toolbar: "P9 · status" | none; the status bar has no phase prefix |
//! | Return band | Settings › Connection (L1) |
//! | handoff band: Paste, Paste image, Select, Copy | the ... menu (L2a) |
//! | tab-and-button band | tab row, ... menu, sidebar header, ⌘ keys (L2a) |
//! | QA bands | QA strip (`qa_strip.rs`, Simulator QA builds only) |
use crate::app::Smoke;

/// Which legacy bands `render_terminal` still draws. L2b turns them off in
/// the same change that adds their new places (plan v2 5.1 rule 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LegacyBands {
    pub(crate) toolbar: bool,
    pub(crate) input_options: bool,
    pub(crate) return_policy: bool,
    pub(crate) handoff: bool,
    pub(crate) actions: bool,
    pub(crate) qa_controls: bool,
}

impl LegacyBands {
    /// Every legacy band is retired (plan v2 7.6).
    pub(crate) const fn current() -> Self {
        Self {
            toolbar: false,
            input_options: false,
            return_policy: false,
            handoff: false,
            actions: false,
            qa_controls: false,
        }
    }

    /// The bands as they were before L2b; tests use it to show what the
    /// retirement removed.
    #[cfg(test)]
    pub(crate) const fn before_l2b() -> Self {
        Self {
            toolbar: true,
            input_options: true,
            return_policy: true,
            handoff: true,
            actions: true,
            qa_controls: true,
        }
    }
}

impl Smoke {
    /// The retired bands, except one: while the command palette is open
    /// (the ... menu's "Command palette", review ab783fbc2a944abc B1) the
    /// actions band slot draws the palette list (`render_actions`), so
    /// `actions.palette` is never true without a list on screen. L6's
    /// Shortcuts popover takes the list over.
    pub(crate) fn legacy_bands(&self) -> LegacyBands {
        LegacyBands {
            actions: self.palette_list_shown(),
            ..LegacyBands::current()
        }
    }

    /// `render_terminal` draws the palette list now (Herdr 1 only: the
    /// palette is a Herdr 1 command, `actions::catalog_context_available`).
    pub(crate) fn palette_list_shown(&self) -> bool {
        self.actions.palette && !self.terminal.core_mode
    }
}

#[cfg(test)]
#[path = "entry_move_tests.rs"]
mod tests;
