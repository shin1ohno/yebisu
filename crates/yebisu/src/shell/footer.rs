//! L2b: the sidebar footer (New space / Hosts / Settings). Owner: L2b.
use super::{ChromeState, EntryPoint, HerdrKind};
use crate::app::Smoke;
use gpui::{AnyElement, Context};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterItem {
    NewSpace,
    Hosts,
    Settings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterEntry {
    pub(crate) item: FooterItem,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

pub(crate) fn footer_items(_kind: HerdrKind) -> Vec<FooterEntry> {
    Vec::new()
}

pub(crate) fn footer_entry_points(_state: &ChromeState) -> Vec<EntryPoint> {
    Vec::new()
}

impl Smoke {
    pub(crate) fn render_sidebar_footer(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}
