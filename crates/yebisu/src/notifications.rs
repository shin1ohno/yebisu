//! Connected-only notification policy. No remote text, file paths or replay queue.
//! Modeled on herdr-gpui 398c132a config/notifications.rs and state.rs:755-777.
use crate::live::Live;
use sheprd_client::protocol::{
    SemanticNotification, SemanticNotificationKind as Kind, SemanticNotificationSound as Sound,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
mod preferences;
pub(crate) use preferences::Edits;
pub(crate) mod sound;

pub(crate) const LIMIT: usize = 32;
pub(crate) const MAX_AGE: Duration = Duration::from_secs(5);
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Disconnected,
    Incoherent,
    InvalidTarget,
    Stale,
    Duplicate,
    Overflow,
    Muted,
    QuietHours,
    Disabled,
    Inactive,
    Permission(Authorization),
    Native,
    Busy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Authorization {
    NotDetermined,
    Denied,
    Authorized,
    Provisional,
    Ephemeral,
    Unknown,
}
#[cfg(target_os = "ios")]
impl From<herdr_uikit::Authorization> for Authorization {
    fn from(value: herdr_uikit::Authorization) -> Self {
        match value {
            herdr_uikit::Authorization::NotDetermined => Self::NotDetermined,
            herdr_uikit::Authorization::Denied => Self::Denied,
            herdr_uikit::Authorization::Authorized => Self::Authorized,
            herdr_uikit::Authorization::Provisional => Self::Provisional,
            herdr_uikit::Authorization::Ephemeral => Self::Ephemeral,
            herdr_uikit::Authorization::Unknown => Self::Unknown,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Off,
    Toast,
    System,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    pub route: Route,
    pub sound: bool,
    pub volume: u8,
    pub muted: bool,
    pub quiet: Option<(u16, u16)>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            route: Route::Off,
            sound: true,
            volume: 50,
            muted: false,
            quiet: None,
        }
    }
}
impl Policy {
    pub fn suppression(&self, minute: u16) -> Option<Failure> {
        if self.muted {
            return Some(Failure::Muted);
        }
        if let Some((start, end)) = self.quiet {
            let inside = if start <= end {
                minute >= start && minute < end
            } else {
                minute >= start || minute < end
            };
            if inside {
                return Some(Failure::QuietHours);
            }
        }
        None
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Context {
    pub host: String,
    pub generation: u64,
    pub boot: String,
    pub pane: Option<String>,
    pub tab: Option<String>,
    pub workspace: Option<String>,
}
#[derive(Clone)]
pub(crate) struct Event {
    pub boot: String,
    pub pane: Option<String>,
    pub tab: Option<String>,
    pub workspace: Option<String>,
    pub kind: Kind,
    pub sound: Option<Sound>,
    pub received: Instant,
}
impl Event {
    pub fn capture(
        live: &Live,
        event: SemanticNotification,
        now: Instant,
    ) -> Result<Self, Failure> {
        let snapshot = current(live)?;
        if [&event.pane_id, &event.tab_id, &event.workspace_id]
            .into_iter()
            .flatten()
            .any(|id| id.is_empty() || id.len() > 128 || id.chars().any(char::is_control))
        {
            return Err(Failure::InvalidTarget);
        }
        let result = Self {
            boot: snapshot.boot_id.clone(),
            pane: event.pane_id,
            tab: event.tab_id,
            workspace: event.workspace_id,
            kind: event.kind,
            sound: event.sound,
            received: now,
        };
        result.context(String::new(), 0).validate(live, "", 0)?;
        Ok(result)
    }
    pub fn context(&self, host: String, generation: u64) -> Context {
        Context {
            host,
            generation,
            boot: self.boot.clone(),
            pane: self.pane.clone(),
            tab: self.tab.clone(),
            workspace: self.workspace.clone(),
        }
    }
    pub fn title(&self) -> &'static str {
        match self.kind {
            Kind::NeedsAttention => "Agent needs attention",
            Kind::Finished => "Agent finished",
            Kind::UpdateInstalled => "Update installed",
            Kind::Custom => "New notification",
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.boot == other.boot
            && self.pane == other.pane
            && self.tab == other.tab
            && self.workspace == other.workspace
            && self.kind == other.kind
            && self.sound == other.sound
    }
}
fn current(live: &Live) -> Result<&sheprd_client::protocol::ClientShellSnapshot, Failure> {
    if !live.connected || live.version_mismatch.get().is_some() {
        return Err(Failure::Disconnected);
    }
    let (snapshot, surface) = live
        .snapshot
        .as_deref()
        .zip(live.surface.as_deref())
        .ok_or(Failure::Incoherent)?;
    if !crate::live::coherent(snapshot, surface) || live.pending_snapshot.is_some() {
        return Err(Failure::Incoherent);
    }
    Ok(snapshot)
}
impl Context {
    pub fn validate(&self, live: &Live, host: &str, generation: u64) -> Result<(), Failure> {
        let snapshot = current(live)?;
        if self.host != host || self.generation != generation || self.boot != snapshot.boot_id {
            return Err(Failure::Stale);
        }
        if let Some(pane) = &self.pane {
            if !snapshot.panes.iter().any(|p| {
                &p.pane_id == pane
                    && self.tab.as_ref().is_none_or(|t| t == &p.tab_id)
                    && self.workspace.as_ref().is_none_or(|w| w == &p.workspace_id)
            }) {
                return Err(Failure::InvalidTarget);
            }
        } else if self.tab.is_some() || self.workspace.is_some() {
            return Err(Failure::InvalidTarget);
        }
        Ok(())
    }
}
#[derive(Clone, Default)]
pub(crate) struct Inbox {
    pub pending: VecDeque<Event>,
    recent: VecDeque<Event>,
    pub failure: Option<Failure>,
}
impl Inbox {
    pub fn push(&mut self, event: Result<Event, Failure>, now: Instant) {
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                self.failure = Some(error);
                return;
            }
        };
        self.recent
            .retain(|old| now.saturating_duration_since(old.received) < Duration::from_secs(2));
        if self.recent.iter().any(|old| old.same(&event)) {
            self.failure = Some(Failure::Duplicate);
            return;
        }
        if self.pending.len() >= LIMIT {
            self.pending.pop_front();
            self.failure = Some(Failure::Overflow);
        }
        if self.recent.len() >= LIMIT {
            self.recent.pop_front();
        }
        self.recent.push_back(event.clone());
        self.pending.push_back(event);
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.recent.clear();
    }
}
#[cfg(test)]
mod tests;
