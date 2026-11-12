//! Safe public-key presentation. Reads and replacement remain host workers.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KeyView {
    Loading,
    Ready(String),
    Failed,
}

impl KeyView {
    /// Accepts only the already-formatted `Reply::Key` payload from setup.
    /// `setup::execute(Job::Key)` is the sole producer and calls
    /// `keys::authorized_line` before creating that reply.
    pub(crate) fn from_formatted_worker_line(line: String) -> Self {
        Self::Ready(line)
    }

    pub(crate) const fn loading() -> Self {
        Self::Loading
    }

    /// A failure carries no dependency error or raw key data into the page.
    pub(crate) const fn failed() -> Self {
        Self::Failed
    }

    pub(crate) fn copy_text(&self, busy: bool) -> Option<&str> {
        match self {
            Self::Ready(line) if !busy => Some(line),
            _ => None,
        }
    }
}
