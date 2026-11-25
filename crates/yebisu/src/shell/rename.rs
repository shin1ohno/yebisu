//! In-place rename of a Herdr 1 space, tab or pane (issue #143).
//!
//! herdr-uikit lays one native `UITextField` over the name
//! (`present_native_inline_field`). While its session exists the SDK's
//! native modal gate keeps keys, composition, Paste and menu commands away
//! from GPUI, as for the search panel; this file is the thin layer around
//! it: when a rename may start (the refusal texts), the rectangle the field
//! covers (`slot`), the session's ordered events, the request (`send`) and
//! the name shown until the host confirms it.
//!
//! Host builds never name `herdr_uikit` (an iOS-only dependency): the types
//! below mirror the SDK's, and `native` converts them on iOS.
use super::HerdrKind;
use crate::{app::Smoke, board::snapshot};
use gpui::{AnyWindowHandle, Bounds, Context, Pixels, Window};
use sheprd_client::{Method, protocol::ClientShellSnapshot};
use std::task::{Context as TaskContext, Poll};

pub(crate) mod entry;
pub(crate) mod labels;
#[cfg(target_os = "ios")]
mod native;
pub(crate) mod normalize;
#[cfg(any(
    test,
    all(target_os = "ios", target_abi = "sim", feature = "test-support")
))]
pub(crate) mod qa;
pub(crate) mod send;
pub(crate) mod slot;

pub(crate) use send::{PendingLabel, RenameTicket};

// keep-sorted start
pub(crate) const AUTOMATIC_NAME: &str = "Automatic name";
pub(crate) const CANCELLED: &str = "Rename cancelled";
pub(crate) const DISCONNECTED: &str = "Host disconnected.";
pub(crate) const EMPTY_HINT: &str = "Enter a name, or press Escape to cancel.";
pub(crate) const FIND_OPEN: &str = "Close Find first";
pub(crate) const HERDR2: &str = "Not available on Herdr 2 yet";
pub(crate) const LENGTH_HINT: &str = "Names are limited to 160 characters.";
pub(crate) const NO_METHOD: &str = "This host can't rename";
pub(crate) const NOT_CONNECTED: &str = "Not connected";
pub(crate) const OTHER_HOST: &str = "Select this host first";
pub(crate) const SHEET_OPEN: &str = "Close the sheet first";
pub(crate) const UNAVAILABLE_HERE: &str = "Rename unavailable here";
pub(crate) const WAITING: &str = "Wait for the last rename";
// keep-sorted end

/// What a rename names, by its Herdr 1 id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RenameObject {
    Pane(String),
    Space(String),
    Tab(String),
}

impl RenameObject {
    pub(crate) fn method(&self) -> Method {
        match self {
            Self::Pane(_) => Method::PaneRename,
            Self::Space(_) => Method::WorkspaceRename,
            Self::Tab(_) => Method::TabRename,
        }
    }

    /// The request's parameters (herdr-gpui's own senders' shape).
    pub(crate) fn params(&self, label: &str) -> serde_json::Value {
        match self {
            Self::Pane(id) => serde_json::json!({ "pane_id": id, "label": label }),
            Self::Space(id) => serde_json::json!({ "workspace_id": id, "label": label }),
            Self::Tab(id) => serde_json::json!({ "tab_id": id, "label": label }),
        }
    }

    /// Only a pane may be named "" (Herdr 1 gives it its automatic name).
    pub(crate) fn allows_empty(&self) -> bool {
        matches!(self, Self::Pane(_))
    }

    pub(crate) fn closed_text(&self) -> &'static str {
        match self {
            Self::Pane(_) => "This pane was closed.",
            Self::Space(_) => "This space was closed.",
            Self::Tab(_) => "This tab was closed.",
        }
    }

    pub(crate) fn exists_in(&self, snapshot: &ClientShellSnapshot) -> bool {
        match self {
            Self::Pane(id) => snapshot.panes.iter().any(|p| &p.pane_id == id),
            Self::Space(id) => snapshot.workspaces.iter().any(|w| &w.workspace_id == id),
            Self::Tab(id) => snapshot.tabs.iter().any(|t| &t.tab_id == id),
        }
    }

    /// The name the host holds now: the field's initial text.
    pub(crate) fn host_name(&self, snapshot: &ClientShellSnapshot) -> String {
        match self {
            Self::Pane(id) => snapshot
                .panes
                .iter()
                .find(|p| &p.pane_id == id)
                .and_then(|p| p.label.clone())
                .unwrap_or_default(),
            Self::Space(id) => snapshot
                .workspaces
                .iter()
                .find(|w| &w.workspace_id == id)
                .map(|w| w.label.clone())
                .unwrap_or_default(),
            Self::Tab(id) => snapshot
                .tabs
                .iter()
                .find(|t| &t.tab_id == id)
                .filter(|t| t.custom_label)
                .map(|t| t.label.clone())
                .unwrap_or_default(),
        }
    }

    /// The field's spoken name: "Rename space session-search".
    pub(crate) fn accessibility_label(&self, snapshot: &ClientShellSnapshot) -> String {
        match self {
            Self::Pane(_) => format!("Rename pane {}", self.host_name(snapshot)),
            Self::Space(_) => format!("Rename space {}", self.host_name(snapshot)),
            Self::Tab(id) => {
                let number = snapshot
                    .tabs
                    .iter()
                    .find(|t| &t.tab_id == id)
                    .map_or(0, |t| t.number);
                format!("Rename tab {number}")
            }
        }
    }
}

/// The element the field covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RenameAnchor {
    AgentRow,
    PaneChip,
    RailHeader,
    SpaceRow,
    TabLabel,
}

/// A rename's object on the host and connection it was started for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenameTarget {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) boot: String,
    pub(crate) generation: u64,
    pub(crate) object: RenameObject,
}

/// Why the field closed (the SDK's `NativeInlineDismissReason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldDismiss {
    // keep-sorted start
    Committed,
    Escape,
    Failed,
    OutsideTap,
    Requested,
    Unchanged,
    WindowClosed,
    // keep-sorted end
}

/// Which GPUI view took first responder back (`RestoredResponder`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldRestored {
    Text,
    Drawing,
    WindowClosed,
    None,
}

/// One event of a session, in the order the SDK sent it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldEvent {
    Opened,
    Committed(String),
    Dismissed(FieldDismiss),
    InputRestored(FieldRestored),
}

/// Why `present` or a later call was refused (`NativeInlineFieldError`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldError {
    NotMainThread,
    WindowUnavailable,
    AlreadyPresented,
    RetirementInProgress,
    Ended,
}

impl FieldError {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::NotMainThread => "Rename could not open (not on the main thread)",
            Self::WindowUnavailable => "Rename could not open (window unavailable)",
            Self::AlreadyPresented => "Rename could not open (another field is open)",
            Self::RetirementInProgress => "Rename could not open (text input is settling)",
            Self::Ended => "Rename ended",
        }
    }
}

/// What yebisu asks of the field; the look is the SDK default's (yebisu's
/// colors), the size the label's.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FieldRequest {
    pub(crate) frame: Bounds<Pixels>,
    pub(crate) clip: Option<Bounds<Pixels>>,
    pub(crate) text: String,
    pub(crate) placeholder: String,
    pub(crate) accessibility_label: String,
    pub(crate) allow_empty: bool,
    pub(crate) font_size: f32,
    /// UIKit's `UIFontWeight`: 0 regular, 0.4 bold.
    pub(crate) font_weight: f32,
}

/// A presented session: its control and its event stream.
pub(crate) type Presented = (Box<dyn FieldControl>, Box<dyn FieldEvents>);

pub(crate) trait FieldPresenter {
    fn present(
        &mut self,
        window: AnyWindowHandle,
        request: FieldRequest,
    ) -> Result<Presented, FieldError>;
}

/// The consumer's hold on a session; dropping it ends the session.
pub(crate) trait FieldControl {
    fn set_frame(
        &self,
        frame: Bounds<Pixels>,
        clip: Option<Bounds<Pixels>>,
    ) -> Result<(), FieldError>;
    fn dismiss(&self) -> Result<(), FieldError>;
}

/// The session's events; ends after `InputRestored`.
pub(crate) trait FieldEvents {
    fn poll_next(&mut self, cx: &mut TaskContext<'_>) -> Poll<Option<FieldEvent>>;
}

/// The SDK's field (iOS); off iOS there is no window to present over.
#[derive(Default)]
pub(crate) struct NativeFieldPresenter;

impl FieldPresenter for NativeFieldPresenter {
    fn present(
        &mut self,
        window: AnyWindowHandle,
        request: FieldRequest,
    ) -> Result<Presented, FieldError> {
        #[cfg(target_os = "ios")]
        {
            native::present(window, request)
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = (window, request);
            Err(FieldError::WindowUnavailable)
        }
    }
}

/// Where a session is. The SDK gate is shut from `Presenting` until
/// `InputRestored`; `Measuring` waits for the anchor's first drawn frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Measuring,
    Presenting,
    Open,
    /// Dismissed or failed: waiting for UIKit to give first responder back.
    Restoring,
}

pub(crate) struct RenameSession {
    /// yebisu's own counter: an event of an older session is ignored.
    pub(crate) id: u64,
    pub(crate) target: RenameTarget,
    pub(crate) anchor: RenameAnchor,
    pub(crate) phase: Phase,
    /// Dropped after `Dismissed`, once a commit has been sent.
    pub(crate) control: Option<Box<dyn FieldControl>>,
    /// `terminal.input.retirement_revision()` right after the open's detach.
    pub(crate) revision_at_open: u64,
    /// Renders in a row that did not draw the anchor (Q-R5).
    pub(crate) missed: u8,
    /// A commit was sent: a session sends at most one request.
    pub(crate) committed: bool,
    /// yebisu asked the field to close; the frame tick waits for `Dismissed`.
    pub(crate) closing: bool,
    /// The rectangle and clip the field was last given.
    pub(crate) placed: Option<slot::Placed>,
    /// The one FIFO receiver of the session's events.
    pub(crate) events: Option<gpui::Task<()>>,
}

impl std::fmt::Debug for RenameSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenameSession")
            .field("id", &self.id)
            .field("target", &self.target)
            .field("anchor", &self.anchor)
            .field("phase", &self.phase)
            .field("holds_control", &self.control.is_some())
            .field("missed", &self.missed)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(crate) struct RenameState {
    pub(crate) session: Option<RenameSession>,
    /// The name sent and shown until the host confirms or refuses it.
    pub(crate) pending: Option<PendingLabel>,
    /// `None` means `NativeFieldPresenter`.
    presenter: Option<Box<dyn FieldPresenter>>,
    next_session: u64,
    /// Where each anchor was drawn, frame by frame.
    pub(crate) slots: slot::Slots,
    /// The tab long-press menu, when open (`tab_row/menu.rs`).
    pub(crate) tab_menu: Option<crate::shell::tab_row::menu::TabMenu>,
    /// The sidebar list's scroll, so a row's field can be scrolled into view.
    pub(crate) sidebar_scroll: gpui::ScrollHandle,
}

impl std::fmt::Debug for RenameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenameState")
            .field("session", &self.session)
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

impl RenameState {
    #[cfg(test)]
    pub(crate) fn set_presenter(&mut self, presenter: Box<dyn FieldPresenter>) {
        self.presenter = Some(presenter);
    }

    /// The pane chips' rename state on `boot`, for the terminal painter.
    pub(crate) fn chip(&self, boot: &str) -> labels::ChipRename {
        let editing = self
            .session
            .as_ref()
            .filter(|s| {
                s.anchor == RenameAnchor::PaneChip
                    && s.phase != Phase::Restoring
                    && s.target.boot == boot
            })
            .and_then(|s| match &s.target.object {
                RenameObject::Pane(id) => Some(id.clone()),
                _ => None,
            });
        let pending = self
            .pending
            .as_ref()
            .filter(|p| p.boot == boot && !p.text.is_empty())
            .and_then(|p| match &p.object {
                RenameObject::Pane(id) => Some((
                    id.clone(),
                    p.text.clone(),
                    if p.sending() {
                        crate::shell::style::color::MUTED
                    } else {
                        crate::shell::style::color::TEXT
                    },
                )),
                _ => None,
            });
        labels::ChipRename {
            slots: self.slots.clone(),
            editing,
            pending,
        }
    }

    /// Whether `object` is being edited now: its GPUI label draws no text.
    pub(crate) fn editing(&self, anchor: RenameAnchor, object: &RenameObject) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.anchor == anchor && &s.target.object == object)
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.phase != Phase::Restoring)
    }
}

impl Smoke {
    /// From `begin_rename` until `InputRestored`, as the SDK gate.
    pub(crate) fn rename_open(&self) -> bool {
        self.shell.rename.session.is_some()
    }

    /// A native field (the search panel's or the rename field) holds first
    /// responder: keys, retirement and the key band leave the terminal alone.
    pub(crate) fn native_field_open(&self) -> bool {
        self.search_open() || self.rename_open()
    }

    /// Why `target` cannot be renamed now (issue §C, in this order), or
    /// `None` when it can.
    pub(crate) fn rename_unavailable(&self, target: &RenameTarget) -> Option<&'static str> {
        if self.input_modal() || self.new_space_open() {
            return Some(SHEET_OPEN);
        }
        if self.find.is_some() {
            return Some(FIND_OPEN);
        }
        if target.kind == HerdrKind::Herdr2 {
            return Some(HERDR2);
        }
        if target.host != self.terminal.reconnect.target
            || target.generation != self.terminal.link.generation
        {
            return Some(OTHER_HOST);
        }
        let live = &self.terminal.live;
        if !live.connected
            || live.version_mismatch.get().is_some()
            || self.terminal.reconnect.suspended
        {
            return Some(NOT_CONNECTED);
        }
        let Some(shown) = snapshot(live) else {
            return Some(NOT_CONNECTED);
        };
        if shown.boot_id != target.boot || !target.object.exists_in(shown) {
            return Some(target.object.closed_text());
        }
        if !target.object.method().advertised_in(&live.methods) {
            return Some(NO_METHOD);
        }
        if self
            .shell
            .rename
            .pending
            .as_ref()
            .is_some_and(|p| p.boot == target.boot && p.object == target.object && p.sending())
        {
            return Some(WAITING);
        }
        None
    }

    /// The selected host's `object` as a rename target.
    pub(crate) fn rename_target(&self, object: RenameObject) -> Option<RenameTarget> {
        let shown = snapshot(&self.terminal.live)?;
        Some(RenameTarget {
            host: self.terminal.reconnect.target.clone(),
            kind: self.terminal.herdr_kind(),
            boot: shown.boot_id.clone(),
            generation: self.terminal.link.generation,
            object,
        })
    }

    /// Starts a rename of `target` over `anchor`; a refusal goes to the
    /// status bar's detail. The field is presented once the anchor has been
    /// drawn (`rename_frame_tick`); the caller asks for that frame.
    pub(crate) fn begin_rename(&mut self, target: RenameTarget, anchor: RenameAnchor) {
        if let Some(reason) = self.rename_unavailable(&target) {
            self.terminal.live.status = reason.into();
            return;
        }
        if self.native_field_open() {
            self.terminal.live.status = FieldError::AlreadyPresented.reason().into();
            return;
        }
        // What the search panel gives up when it opens: composition is
        // dropped, a Paste being prepared is cancelled, no palette.
        self.actions.palette = false;
        self.actions.consumed.clear();
        self.terminal.input.detach();
        let revision_at_open = self.terminal.input.retirement_revision();
        self.shell.rename.next_session += 1;
        self.shell.rename.session = Some(RenameSession {
            id: self.shell.rename.next_session,
            target,
            anchor,
            phase: Phase::Measuring,
            control: None,
            revision_at_open,
            missed: 0,
            committed: false,
            closing: false,
            placed: None,
            events: None,
        });
        // Registered first: `sync_handoff` now yields no stamp.
        self.handoff.sync(None);
        self.handoff.cancel();
        self.handoff.select_mode = false;
    }

    /// Presents the field over `placed` (deferred out of the render that
    /// first drew the anchor).
    pub(crate) fn present_rename(
        &mut self,
        id: u64,
        placed: slot::Placed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self
            .shell
            .rename
            .session
            .as_ref()
            .filter(|s| s.id == id && s.phase == Phase::Measuring)
        else {
            return;
        };
        let target = session.target.clone();
        let anchor = session.anchor;
        let Some(shown) = snapshot(&self.terminal.live) else {
            self.end_rename(id, Some(target.object.closed_text()), cx);
            return;
        };
        let (font_size, font_weight) = slot::font(anchor);
        let request = FieldRequest {
            frame: placed.frame,
            clip: placed.clip,
            text: target.object.host_name(shown),
            placeholder: if target.object.allows_empty() {
                AUTOMATIC_NAME.into()
            } else {
                target.object.host_name(shown)
            },
            accessibility_label: target.object.accessibility_label(shown),
            allow_empty: target.object.allows_empty(),
            font_size,
            font_weight,
        };
        let presented = match self.shell.rename.presenter.as_mut() {
            Some(presenter) => presenter.present(window.window_handle(), request),
            None => NativeFieldPresenter.present(window.window_handle(), request),
        };
        let (control, mut events) = match presented {
            Ok(session) => session,
            Err(error) => {
                self.end_rename(id, Some(error.reason()), cx);
                return;
            }
        };
        let receiver = cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = std::future::poll_fn(|task| events.poll_next(task)).await {
                if this
                    .update_in(cx, |app, window, cx| {
                        app.on_rename_event(id, event, window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        if let Some(session) = self.shell.rename.session.as_mut() {
            session.control = Some(control);
            session.phase = Phase::Presenting;
            session.placed = Some(placed);
            session.events = Some(receiver);
        }
        cx.notify();
    }

    /// Ends a session that never reached the SDK, with `reason` shown.
    fn end_rename(&mut self, id: u64, reason: Option<&'static str>, cx: &mut Context<Self>) {
        if self.shell.rename.session.as_ref().map(|s| s.id) == Some(id) {
            self.shell.rename.session = None;
            if let Some(reason) = reason {
                self.terminal.live.status = reason.into();
            }
            self.refresh_terminal_input();
        }
        cx.notify();
    }

    pub(crate) fn on_rename_event(
        &mut self,
        id: u64,
        event: FieldEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.rename.session.as_ref().map(|s| s.id) != Some(id) {
            return;
        }
        match event {
            FieldEvent::Opened => self.with_rename(|s| s.phase = Phase::Open),
            FieldEvent::Committed(text) => {
                let target = self.shell.rename.session.as_mut().and_then(|s| {
                    (!std::mem::replace(&mut s.committed, true)).then(|| s.target.clone())
                });
                if let Some(target) = target {
                    self.send_rename(&target, &text, cx);
                }
            }
            FieldEvent::Dismissed(reason) => {
                if reason == FieldDismiss::Failed {
                    self.terminal.live.status = "Rename could not open".into();
                }
                // The handle goes after the commit was sent (`Committed`
                // came first), so the terminal never takes a key between.
                self.with_rename(|s| {
                    s.control = None;
                    s.phase = Phase::Restoring;
                });
            }
            FieldEvent::InputRestored(restored) => {
                let Some(session) = self.shell.rename.session.take() else {
                    return;
                };
                // The rail menu stayed open for its header; it closes now.
                if session.anchor == RenameAnchor::RailHeader {
                    self.dismiss_rail_menu();
                }
                if restored == FieldRestored::Text {
                    self.terminal
                        .input
                        .release_after_native_search(session.revision_at_open);
                }
                self.refresh_terminal_input();
                window.notify_text_input_changed(&self.terminal_focus);
            }
        }
        cx.notify();
    }

    fn with_rename(&mut self, change: impl FnOnce(&mut RenameSession)) {
        if let Some(session) = self.shell.rename.session.as_mut() {
            change(session);
        }
    }

    /// Closes the open field (the consumer's dismissal); nothing is sent.
    pub(crate) fn dismiss_rename(&mut self, reason: &'static str) {
        let Some(session) = self.shell.rename.session.as_ref() else {
            return;
        };
        let id = session.id;
        if session.phase == Phase::Measuring {
            self.shell.rename.session = None;
            self.terminal.live.status = reason.into();
            self.refresh_terminal_input();
            return;
        }
        if session.closing {
            return;
        }
        if let Some(control) = session.control.as_ref()
            && let Err(error) = control.dismiss()
        {
            tracing::warn!(?error, id, "rename field not dismissed");
        }
        self.with_rename(|s| s.closing = true);
        self.terminal.live.status = reason.into();
    }
}

#[cfg(test)]
#[path = "rename_tests.rs"]
pub(crate) mod tests;
