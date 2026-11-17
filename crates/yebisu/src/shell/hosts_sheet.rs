//! L7: the Hosts sheet and the first-run screen (S-14). Owner: L7.
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

#[derive(Debug, Default)]
pub(crate) struct HostsSheetState {}

impl Smoke {
    /// Footer Hosts. `false` means not handled: the caller keeps today's
    /// "Keys and hosts" page.
    pub(crate) fn open_hosts(&mut self, _cx: &mut Context<Self>) -> bool {
        false
    }

    pub(crate) fn render_hosts_sheet(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }

    /// With zero hosts: the empty frame plus the Add host sheet. `None`
    /// keeps today's setup page.
    pub(crate) fn render_first_run(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }
}
