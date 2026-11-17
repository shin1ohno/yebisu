//! L7: the three-step Add host sheet. Owner: L7.
//!
//! The model is pure: it holds what each step may do and what the sheet says
//! after a failure. The sheet view and the background jobs live in
//! `hosts_sheet`; they call these methods and never decide a rule themselves.
use super::HerdrKind;
use crate::layout::OVERLAY_MARGIN;

/// Width of the Add host sheet on a wide window (design text, sheet sizes).
pub(crate) const SHEET_WIDTH: f32 = 620.0;

/// `min(620, window width - 32)`; never negative.
pub(crate) fn sheet_width(window_width: f32) -> f32 {
    (window_width - OVERLAY_MARGIN).clamp(0.0, SHEET_WIDTH)
}

/// Sheet height that stays above the software keyboard.
pub(crate) fn sheet_height(window_height: f32, keyboard_height: f32) -> f32 {
    (window_height - keyboard_height - OVERLAY_MARGIN).max(0.0)
}

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
    /// A host-key job (a scan or a trust) is in flight.
    pub(crate) scanning: bool,
    pub(crate) scanned_fingerprint: Option<String>,
}

/// What a finished scan found, already reduced to what the sheet needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ScanOutcome {
    /// The host offered a key nobody has trusted yet.
    New {
        fingerprint: String,
    },
    /// The key is already pinned; there is nothing to compare.
    Pinned {
        fingerprint: String,
    },
    /// The host offered a key that differs from the pinned one.
    Mismatch {
        expected: String,
        seen: String,
    },
    Failed(String),
}

impl AddHostFlow {
    pub(crate) fn can_scan(&self) -> bool {
        !self.scanning && self.step != AddHostStep::SignIn && !self.typed_host.trim().is_empty()
    }

    /// Trust only after a successful scan (`app_setup.rs:181-186`). Editing
    /// the host clears the scan, so the fingerprint on screen always belongs
    /// to the host in the field.
    pub(crate) fn can_trust(&self) -> bool {
        !self.scanning && self.step == AddHostStep::Trust && self.scanned_fingerprint.is_some()
    }

    /// The host field changed. A scan result never outlives the text it was
    /// taken for. Ignored while a job runs and after the host is trusted.
    pub(crate) fn edit_host(&mut self, text: &str) -> bool {
        if self.scanning || self.step == AddHostStep::SignIn || self.typed_host == text {
            return false;
        }
        self.typed_host = text.to_owned();
        self.scanned_fingerprint = None;
        self.step = AddHostStep::Address;
        true
    }

    /// Start a scan; returns the host to scan, or `None` when not allowed.
    pub(crate) fn begin_scan(&mut self) -> Option<String> {
        if !self.can_scan() {
            return None;
        }
        self.scanning = true;
        self.scanned_fingerprint = None;
        self.step = AddHostStep::Address;
        Some(self.typed_host.trim().to_owned())
    }

    /// Apply a scan reply. A reply for another host, or when no scan runs,
    /// is stale and changes nothing. The typed host is kept on every outcome.
    pub(crate) fn finish_scan(&mut self, target: &str, outcome: &ScanOutcome) -> bool {
        if !self.scanning || self.step != AddHostStep::Address || target != self.typed_host.trim() {
            return false;
        }
        self.scanning = false;
        match outcome {
            ScanOutcome::New { fingerprint } => {
                self.scanned_fingerprint = Some(fingerprint.clone());
                self.step = AddHostStep::Trust;
            }
            ScanOutcome::Pinned { fingerprint } => {
                self.scanned_fingerprint = Some(fingerprint.clone());
                self.step = AddHostStep::SignIn;
            }
            ScanOutcome::Mismatch { .. } | ScanOutcome::Failed(_) => {
                self.scanned_fingerprint = None;
                self.step = AddHostStep::Address;
            }
        }
        true
    }

    /// Cancel the running scan. Its late reply is dropped by the caller.
    pub(crate) fn cancel_scan(&mut self) -> bool {
        let running = self.scanning && self.step == AddHostStep::Address;
        if running {
            self.scanning = false;
        }
        running
    }

    /// The compared fingerprint was approved. `false` when Trust is not allowed.
    pub(crate) fn begin_trust(&mut self) -> bool {
        if !self.can_trust() {
            return false;
        }
        self.scanning = true;
        true
    }

    pub(crate) fn finish_trust(&mut self, trusted: bool) {
        if !(self.scanning && self.step == AddHostStep::Trust) {
            return;
        }
        self.scanning = false;
        if trusted {
            self.step = AddHostStep::SignIn;
        } else {
            self.scanned_fingerprint = None;
            self.step = AddHostStep::Address;
        }
    }

    /// "Not this host": back to the address with the text kept.
    pub(crate) fn reject_key(&mut self) {
        if !self.scanning && self.step == AddHostStep::Trust {
            self.scanned_fingerprint = None;
            self.step = AddHostStep::Address;
        }
    }
}

/// What a probe of the host reports about Herdr, as plain facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Probe {
    /// A Herdr 1 that this client can use is installed.
    pub(crate) herdr1: bool,
    /// A Herdr 2 daemon is available.
    pub(crate) herdr2: bool,
    /// A Herdr is installed but too old for this client.
    pub(crate) outdated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SignInOutcome {
    Open(HerdrKind),
    Outdated,
    NoHerdr,
}

/// Step 3 opens Herdr 1 first and Herdr 2 only when Herdr 1 is not usable.
pub(crate) fn sign_in_outcome(probe: Probe) -> SignInOutcome {
    if probe.herdr1 {
        SignInOutcome::Open(HerdrKind::Herdr1)
    } else if probe.herdr2 {
        SignInOutcome::Open(HerdrKind::Herdr2)
    } else if probe.outdated {
        SignInOutcome::Outdated
    } else {
        SignInOutcome::NoHerdr
    }
}

/// The message under a step after something went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    ScanFailed(String),
    Mismatch { expected: String, seen: String },
    TrustFailed(String),
    SignInFailed(String),
    NoHerdr,
    HerdrOutdated,
    Cancelled,
}

impl Notice {
    /// Lines the sheet prints; a mismatch prints both fingerprints.
    pub(crate) fn lines(&self) -> Vec<String> {
        match self {
            Self::ScanFailed(reason) => vec![format!("Could not check the host key: {reason}")],
            Self::Mismatch { expected, seen } => vec![
                "The host key changed. Refused; nothing was trusted.".into(),
                format!("Expected: {expected}"),
                format!("Seen: {seen}"),
            ],
            Self::TrustFailed(reason) => vec![format!("Could not trust the host key: {reason}")],
            Self::SignInFailed(reason) => vec![format!("Sign-in failed: {reason}")],
            Self::NoHerdr => vec!["SSH connected, but no Herdr answered on the host.".into()],
            Self::HerdrOutdated => vec!["Herdr on the host needs an update.".into()],
            Self::Cancelled => vec!["Scan cancelled.".into()],
        }
    }

    /// Mismatch and cancel have no Retry: the first is refused, the second is chosen.
    pub(crate) fn offers_retry(&self) -> bool {
        !matches!(self, Self::Mismatch { .. } | Self::Cancelled)
    }
}

/// Why the sheet is open: a new host, a saved host address, or opening a
/// saved host (its key is already pinned, so it starts at sign-in).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SheetMode {
    #[default]
    Add,
    Edit,
    Open,
}

impl SheetMode {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Add => "Add host",
            Self::Edit => "Edit host",
            Self::Open => "Open host",
        }
    }
}

/// The flow plus what the sheet shows after a failure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AddHostSession {
    pub(crate) mode: SheetMode,
    pub(crate) flow: AddHostFlow,
    pub(crate) notice: Option<Notice>,
    pub(crate) signing_in: bool,
}

impl AddHostSession {
    pub(crate) fn new(mode: SheetMode, host: &str) -> Self {
        let step = match mode {
            SheetMode::Open => AddHostStep::SignIn,
            SheetMode::Add | SheetMode::Edit => AddHostStep::Address,
        };
        Self {
            mode,
            flow: AddHostFlow {
                step,
                typed_host: host.to_owned(),
                ..AddHostFlow::default()
            },
            ..Self::default()
        }
    }

    pub(crate) fn can_sign_in(&self) -> bool {
        self.flow.step == AddHostStep::SignIn && !self.signing_in
    }

    pub(crate) fn begin_sign_in(&mut self) -> Option<String> {
        if !self.can_sign_in() {
            return None;
        }
        self.signing_in = true;
        self.notice = None;
        Some(self.flow.typed_host.trim().to_owned())
    }

    /// Returns the Herdr to open, or records why none opens.
    pub(crate) fn finish_sign_in(&mut self, result: Result<Probe, String>) -> Option<HerdrKind> {
        if !self.signing_in {
            return None;
        }
        self.signing_in = false;
        match result {
            Err(reason) => self.notice = Some(Notice::SignInFailed(reason)),
            Ok(probe) => match sign_in_outcome(probe) {
                SignInOutcome::Open(kind) => return Some(kind),
                SignInOutcome::Outdated => self.notice = Some(Notice::HerdrOutdated),
                SignInOutcome::NoHerdr => self.notice = Some(Notice::NoHerdr),
            },
        }
        None
    }

    pub(crate) fn begin_scan(&mut self) -> Option<String> {
        let target = self.flow.begin_scan()?;
        self.notice = None;
        Some(target)
    }

    pub(crate) fn finish_scan(&mut self, target: &str, outcome: ScanOutcome) {
        if !self.flow.finish_scan(target, &outcome) {
            return;
        }
        self.notice = match outcome {
            ScanOutcome::Failed(reason) => Some(Notice::ScanFailed(reason)),
            ScanOutcome::Mismatch { expected, seen } => Some(Notice::Mismatch { expected, seen }),
            ScanOutcome::New { .. } | ScanOutcome::Pinned { .. } => None,
        };
    }

    pub(crate) fn cancel_scan(&mut self) {
        if self.flow.cancel_scan() {
            self.notice = Some(Notice::Cancelled);
        }
    }

    pub(crate) fn finish_trust(&mut self, result: Result<(), String>) {
        if !(self.flow.scanning && self.flow.step == AddHostStep::Trust) {
            return;
        }
        self.flow.finish_trust(result.is_ok());
        if let Err(reason) = result {
            self.notice = Some(Notice::TrustFailed(reason));
        }
    }
}

#[cfg(test)]
#[path = "add_host_tests.rs"]
mod tests;
