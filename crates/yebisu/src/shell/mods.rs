//! L9: the Mods status item and overlay (Q3). Owner: L9.
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

/// "Mods n" only when `withdrawn` > 0.
pub(crate) fn mods_item(_withdrawn: usize) -> Option<String> {
    None
}

impl Smoke {
    pub(crate) fn render_mods_overlay(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }
}
