//! L2b: the Simulator-only QA strip (plan v2 5.3). Owner: L2b.
use crate::app::Smoke;
use gpui::{AnyElement, Context};

/// True only in a Simulator build with a QA environment variable set.
pub(crate) fn qa_strip_enabled() -> bool {
    false
}

impl Smoke {
    pub(crate) fn render_qa_strip(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}
