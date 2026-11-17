//! L6: Host / Space menus in the menu bar and on long press (Q8). Owner: L6.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostMenuItem {
    Reconnect,
    Detach,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpaceMenuItem {
    NewTab,
    Detach,
}

/// The target captured when a menu was shown (S-10).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MenuScope {
    pub(crate) host: String,
    pub(crate) space: Option<String>,
}

/// Whether an item shown for `shown` may still run against `current`.
/// Wave 0 refuses everything; nothing calls it yet.
pub(crate) fn scope_still_valid(_shown: &MenuScope, _current: &MenuScope) -> bool {
    false
}
