//! L8: Herdr 2 in the shared frame. Owner: L8.
use super::HerdrKind;
use crate::app::Smoke;

#[derive(Debug, Default)]
pub(crate) struct Herdr2State {}

/// One connection per (host, kind).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ConnectionKey {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
}

impl Smoke {
    /// Whether the Herdr 2 control band (CB) stays in the terminal screen.
    /// L8 returns false once the ... menu and pane taps carry it.
    pub(crate) fn core_controls_in_band(&self) -> bool {
        true
    }
}
