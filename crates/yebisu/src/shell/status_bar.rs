//! L2a: the one-line status bar. Owner: L2a.
//! Text from other streams comes through their own functions:
//! L5 `edit_bar::paused_typing_status`, L9 `fit::status_bar_overflow`,
//! L9 `mods::mods_item`, L6 `Smoke::open_shortcuts`.
use super::{ChromeState, HerdrKind};
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StatusBarModel {
    pub(crate) connection: String,
    pub(crate) kind: HerdrKind,
    pub(crate) state: String,
    pub(crate) show_reconnect: bool,
    pub(crate) show_sidebar_hint: bool,
    pub(crate) accessibility_label: String,
}

pub(crate) fn status_bar_model(_state: &ChromeState, _connection: &str) -> StatusBarModel {
    StatusBarModel::default()
}

impl Smoke {
    pub(crate) fn render_status_bar(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }
}
