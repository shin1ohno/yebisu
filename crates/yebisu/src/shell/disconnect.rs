//! L9: the 30 s fade of a disconnected region (mod point 7). Owner: L9.
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DisconnectFade {
    Visible,
    Dimmed,
    Cleared,
}

pub(crate) fn disconnect_fade(_elapsed: Duration) -> DisconnectFade {
    DisconnectFade::Visible
}
