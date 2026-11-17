//! L3: hosts > spaces > AGENTS. Owner: L3.
use super::{HerdrKind, sidebar_status::SidebarStatus};
use crate::app::Smoke;
use gpui::{AnyElement, Context};

#[derive(Debug, Default)]
pub(crate) struct SidebarState {}

/// (host, kind, pane id or address), D8.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct AgentIdentity {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) pane: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentRow {
    pub(crate) identity: AgentIdentity,
    pub(crate) name: String,
    pub(crate) tab: u64,
    pub(crate) status: SidebarStatus,
    pub(crate) received: u64,
}

/// Q5 order, newest received first within one status.
pub(crate) fn order_agents(_rows: &mut [AgentRow]) {}

/// Shown name: a tab/pane suffix only when names collide.
pub(crate) fn display_name(row: &AgentRow, _all: &[AgentRow]) -> String {
    row.name.clone()
}

/// Spoken name: always includes tab and pane (S-8).
pub(crate) fn accessibility_name(row: &AgentRow) -> String {
    row.name.clone()
}

/// Row height for a text scale (1.0 = default), S-11.
pub(crate) fn row_height(base: f32, _text_scale: f32) -> f32 {
    base
}

impl Smoke {
    /// The sidebar list; `None` keeps the legacy board in the column.
    pub(crate) fn render_sidebar_list(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}
