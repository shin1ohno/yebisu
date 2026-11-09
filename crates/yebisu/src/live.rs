//! UI-independent reducer. Only a coherent metadata/surface pair is displayed.
use sheprd_client::{
    ClientEvent, VersionMismatch,
    protocol::{ClientShellSnapshot, PaneSurfaceFrame, ServerMessage},
};
use std::{
    collections::VecDeque,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Default)]
pub(crate) struct Live {
    pub connected: bool,
    /// Preserves target changes even when the latest-state mailbox skips a frame.
    pub input_epoch: u64,
    pub snapshot: Option<Arc<ClientShellSnapshot>>,
    pub pending_snapshot: Option<Arc<ClientShellSnapshot>>,
    pub surface: Option<Arc<PaneSurfaceFrame>>,
    future_surface: Option<Arc<PaneSurfaceFrame>>,
    pub keyboard_report_all: bool,
    pub status: String,
    /// Shared with Link and retired inboxes; UI polling cannot lose a stop.
    pub version_mismatch: Arc<OnceLock<VersionMismatch>>,
    pub capabilities: Vec<String>,
    pub version: String,
    pub boot: Option<String>,
    pub clipboard: VecDeque<String>,
    pub pending_request: Option<String>,
    pub failed_request: Option<String>,
    pub dirty: bool,
}

pub(crate) fn coherent(snapshot: &ClientShellSnapshot, surface: &PaneSurfaceFrame) -> bool {
    snapshot.boot_id == surface.boot_id && snapshot.revision == surface.projection_revision
}

impl Live {
    pub fn apply(&mut self, event: ClientEvent) {
        let before = crate::terminal_input::destination(self, 0);
        self.dirty = true;
        match event {
            ClientEvent::Connected(welcome) => {
                self.connected = true;
                self.capabilities = welcome.capabilities;
                self.version = welcome.server_version;
                self.status = "Connected".into();
            }
            ClientEvent::Snapshot(snapshot) => self.receive_snapshot(snapshot),
            ClientEvent::Surface(surface) => self.receive_surface(surface),
            ClientEvent::SurfaceImages(_) => {} // P8 restores the image catalog here.
            ClientEvent::Response {
                request_id,
                response,
            } => {
                if self.pending_request.as_ref() == Some(&request_id) {
                    self.pending_request = None;
                    if let Some(error) = response.get("error") {
                        self.failed_request = Some(request_id);
                        self.status = format!("Navigation rejected: {error}");
                    }
                }
            }
            ClientEvent::CommandRejected { request_id, reason } => {
                if request_id == self.pending_request {
                    self.failed_request = request_id;
                    self.pending_request = None;
                }
                self.status = reason.to_string();
            }
            ClientEvent::VersionMismatch(mismatch) => {
                self.status = format!("Version mismatch: {mismatch:?}");
                let _ = self.version_mismatch.set(mismatch);
                self.surface = None;
            }
            ClientEvent::Disconnected { reason, ssh: _ } => {
                self.connected = false;
                if self.version_mismatch.get().is_none() {
                    self.status = format!("Disconnected: {reason}");
                }
                self.surface = None;
                self.future_surface = None;
                self.pending_snapshot = None;
                self.keyboard_report_all = false;
                self.clipboard.clear();
            }
            ClientEvent::Message(message) => match message {
                ServerMessage::ClientShellKeyboardReportAll { enabled } => {
                    self.keyboard_report_all = enabled
                }
                ServerMessage::Clipboard { data } => {
                    if let Some(text) = crate::osc52::decode(&data) {
                        if self.clipboard.len() == crate::osc52::MAX_PENDING {
                            self.clipboard.pop_front();
                        }
                        self.clipboard.push_back(text);
                    }
                }
                ServerMessage::Welcome { .. }
                | ServerMessage::Terminal(_)
                | ServerMessage::Graphics { .. }
                | ServerMessage::ServerShutdown { .. }
                | ServerMessage::Notify { .. }
                | ServerMessage::WindowTitle { .. }
                | ServerMessage::ReloadSoundConfig
                | ServerMessage::MouseCapture { .. }
                | ServerMessage::TerminalBell { .. }
                | ServerMessage::GraphicsFile { .. }
                | ServerMessage::GraphicsTransmissionRetired { .. }
                | ServerMessage::ClientShellSnapshot(_)
                | ServerMessage::PaneSurface(_)
                | ServerMessage::SemanticNotification(_)
                | ServerMessage::ClientShellError { .. }
                | ServerMessage::DirectTerminalKeyboardProtocol { .. }
                | ServerMessage::ClientShellEndpointResponseChunk { .. }
                | ServerMessage::PaneSurfacePatch(_)
                | ServerMessage::EndpointControl { .. } => {}
            },
        }
        if before != crate::terminal_input::destination(self, 0) {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
    }

    fn receive_snapshot(&mut self, snapshot: Arc<ClientShellSnapshot>) {
        if self
            .boot
            .as_ref()
            .is_some_and(|boot| *boot != snapshot.boot_id)
        {
            return;
        }
        self.boot = Some(snapshot.boot_id.clone());
        if self
            .pending_snapshot
            .as_ref()
            .or(self.snapshot.as_ref())
            .is_some_and(|old| old.revision > snapshot.revision)
        {
            return;
        }
        if let Some(future) = self.future_surface.take() {
            if coherent(&snapshot, &future) {
                self.snapshot = Some(snapshot);
                self.pending_snapshot = None;
                self.surface = Some(future);
                return;
            }
            if future.boot_id == snapshot.boot_id && future.projection_revision > snapshot.revision
            {
                self.future_surface = Some(future);
            }
        }
        if self
            .surface
            .as_ref()
            .zip(self.snapshot.as_ref())
            .is_some_and(|(surface, old)| {
                coherent(old, surface)
                    && same_input_projection(old, &snapshot)
                    && surface.projection_revision != snapshot.revision
            })
        {
            self.pending_snapshot = Some(snapshot);
            return;
        }
        if self
            .surface
            .as_ref()
            .is_some_and(|surface| !coherent(&snapshot, surface))
        {
            self.surface = None;
        }
        self.pending_snapshot = None;
        self.snapshot = Some(snapshot);
    }

    fn receive_surface(&mut self, surface: Arc<PaneSurfaceFrame>) {
        if self
            .boot
            .as_ref()
            .is_some_and(|boot| *boot != surface.boot_id)
        {
            return;
        }
        let latest = self.pending_snapshot.as_ref().or(self.snapshot.as_ref());
        if latest.is_none_or(|s| surface.projection_revision > s.revision) {
            if self.future_surface.as_ref().is_none_or(|old| {
                (surface.projection_revision, surface.surface_revision)
                    > (old.projection_revision, old.surface_revision)
            }) {
                self.future_surface = Some(surface);
            }
            return;
        }
        if let Some(pending) = &self.pending_snapshot {
            if !coherent(pending, &surface) {
                return;
            }
            self.snapshot = self.pending_snapshot.take();
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|s| coherent(s, &surface))
            && self.surface.as_ref().is_none_or(|old| {
                old.projection_revision != surface.projection_revision
                    || old.surface_revision < surface.surface_revision
            })
        {
            self.surface = Some(surface);
        }
    }
}

fn same_input_projection(old: &ClientShellSnapshot, new: &ClientShellSnapshot) -> bool {
    // New protocol fields must make an explicit choice about input invalidation.
    let ClientShellSnapshot {
        boot_id,
        revision: _,
        config_diagnostic: _,
        product_announcement: _,
        update_available: _,
        update_install_command: _,
        server_keybindings_toml,
        latest_release_notes_available: _,
        integration_updates_available: _,
        worktree_directory: _,
        release_notes: _,
        focused_workspace_id,
        focused_tab_id,
        focused_pane_id,
        tab_bar_right: _,
        tab_bar_right_separator: _,
        agent_view_label: _,
        agent_order: _,
        workspaces: _,
        tabs: _,
        panes: _,
        agents: _,
        commands,
    } = new;
    if &old.boot_id != boot_id
        || &old.focused_workspace_id != focused_workspace_id
        || &old.focused_tab_id != focused_tab_id
        || &old.focused_pane_id != focused_pane_id
        || &old.server_keybindings_toml != server_keybindings_toml
        || &old.commands != commands
    {
        return false;
    }
    let zoomed = |snapshot: &ClientShellSnapshot| {
        snapshot
            .tabs
            .iter()
            .find(|tab| Some(&tab.tab_id) == snapshot.focused_tab_id.as_ref())
            .map(|tab| tab.zoomed)
    };
    zoomed(old) == zoomed(new) && input_panes(old).eq(input_panes(new))
}

fn input_panes(
    snapshot: &ClientShellSnapshot,
) -> impl Iterator<Item = (&str, &str, &str, bool, bool)> {
    snapshot
        .panes
        .iter()
        .filter(move |pane| Some(&pane.tab_id) == snapshot.focused_tab_id.as_ref())
        .map(|pane| {
            (
                pane.pane_id.as_str(),
                pane.workspace_id.as_str(),
                pane.tab_id.as_str(),
                pane.focused,
                pane.right_click_passthrough,
            )
        })
}

#[cfg(test)]
mod tests;
