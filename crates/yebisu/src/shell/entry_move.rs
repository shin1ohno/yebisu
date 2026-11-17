//! L2b: retires the legacy bands. Owner: L2b.
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
    pub(crate) const fn current() -> Self {
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
    pub(crate) fn legacy_bands(&self) -> LegacyBands {
        LegacyBands::current()
    }
}
