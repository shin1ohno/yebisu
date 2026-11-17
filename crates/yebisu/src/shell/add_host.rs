//! L7: the three-step Add host sheet. Owner: L7.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AddHostStep {
    #[default]
    Address,
    Trust,
    SignIn,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AddHostFlow {
    pub(crate) step: AddHostStep,
    pub(crate) typed_host: String,
    pub(crate) scanning: bool,
    pub(crate) scanned_fingerprint: Option<String>,
}

impl AddHostFlow {
    pub(crate) fn can_scan(&self) -> bool {
        false
    }

    /// Trust only after a successful scan (`app_setup.rs:181-186`).
    pub(crate) fn can_trust(&self) -> bool {
        false
    }
}
