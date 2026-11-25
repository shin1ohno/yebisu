//! Sidebar search (R24): the native search panel the magnifier opens.
//!
//! herdr-uikit presents a UIKit search field and results over the window
//! (`present_native_search`). While its session exists the SDK's native
//! modal gate keeps keys, composition, Paste and dictation away from GPUI;
//! this file is the thin layer around it: when the panel may open, what the
//! terminal gives up before it opens, the session's ordered events, the
//! result rows and how input comes back after it closed (issue #138).
//!
//! Host builds never name `herdr_uikit` (an iOS-only dependency): the types
//! below mirror the SDK's, and the default presenter converts them on iOS.
use super::filter;
use crate::app::Smoke;
use gpui::{AnyWindowHandle, Bounds, Context, Pixels, Window, px, size};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context as TaskContext, Poll},
};

pub(crate) mod results;

pub(crate) const TITLE: &str = "Search spaces and agents";
pub(crate) const CANCEL: &str = "Cancel";
pub(crate) const CANCEL_LABEL: &str = "Cancel search";
pub(crate) const PLACEHOLDER: &str = "Name or host";
pub(crate) const HINT: &str = "Type a space, agent or host name.";
pub(crate) const UNAVAILABLE: &str = "This space is no longer available.";
pub(crate) const POPOVER_WIDTH: f32 = 360.;
pub(crate) const POPOVER_HEIGHT: f32 = 560.;
/// The popover never runs closer than this to the window's edges.
pub(crate) const EDGE: f32 = 24.;
/// The magnifier's touch size, the anchor before it was first painted.
const ANCHOR_FALLBACK: f32 = 44.;

/// How the panel appears; fixed for a session when it opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Presentation {
    #[default]
    Popover,
    Sheet,
}

/// Why the panel closed (the SDK's `NativeSearchDismissReason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DismissReason {
    // keep-sorted start
    Cancel,
    Escape,
    OutsideTap,
    Requested,
    Return,
    Selected(u64),
    SwipedDown,
    System,
    WindowClosed,
    // keep-sorted end
}

/// Why the panel never became usable (`NativeSearchFailure`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailReason {
    FieldRefused,
    PresenterBusy,
    WindowDetached,
}

/// Which GPUI view took first responder back (`RestoredResponder`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Restored {
    Text,
    Drawing,
    WindowClosed,
    None,
}

/// Why `present` or a later call was refused (`NativeSearchError`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PresentError {
    NotMainThread,
    WindowUnavailable,
    AlreadyPresented,
    RetirementInProgress,
    Ended,
}

impl PresentError {
    /// The status line's text when the panel could not open.
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::NotMainThread => "Search could not open (not on the main thread)",
            Self::WindowUnavailable => "Search could not open (window unavailable)",
            Self::AlreadyPresented => "Search is already open",
            Self::RetirementInProgress => "Search could not open (text input is settling)",
            Self::Ended => "Search ended",
        }
    }
}

/// One event of a session, in the order the SDK sent it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SearchEvent {
    Opened,
    Failed(FailReason),
    QueryChanged(String),
    ClearRequested,
    Selected { id: u64, revision: u64 },
    Dismissed(DismissReason),
    InputRestored(Restored),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SearchRequest {
    pub(crate) anchor: Bounds<Pixels>,
    pub(crate) presentation: Presentation,
    pub(crate) title: String,
    pub(crate) cancel_label: String,
    pub(crate) cancel_accessibility_label: String,
    pub(crate) placeholder: String,
    pub(crate) query: String,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) results: SearchResults,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SearchResults {
    pub(crate) status: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) sections: Vec<SearchSection>,
    pub(crate) empty: Option<SearchEmpty>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SearchSection {
    pub(crate) title: String,
    pub(crate) items: Vec<SearchItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SearchItem {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) detail: String,
    /// The status dot's color (`0xrrggbb`).
    pub(crate) dot: Option<u32>,
    pub(crate) accessibility_label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SearchEmpty {
    pub(crate) message: String,
    pub(crate) clear_label: String,
}

/// A presented session: its control and its event stream.
pub(crate) type Presented = (Box<dyn SearchControl>, Box<dyn SearchEvents>);

/// Presents a panel and hands back its control and its event stream.
pub(crate) trait SearchPresenter {
    fn present(
        &mut self,
        window: AnyWindowHandle,
        request: SearchRequest,
    ) -> Result<Presented, PresentError>;
}

/// The consumer's hold on a session; dropping it ends the session.
pub(crate) trait SearchControl {
    fn update(&self, results: SearchResults) -> Result<u64, PresentError>;
    fn dismiss_selected(&self, id: u64) -> Result<(), PresentError>;
    fn set_query(&self, query: &str) -> Result<(), PresentError>;
}

/// The session's events; ends after `InputRestored`.
pub(crate) trait SearchEvents {
    fn poll_next(&mut self, cx: &mut TaskContext<'_>) -> Poll<Option<SearchEvent>>;
}

/// The SDK's panel (iOS); off iOS there is no window to present over.
#[derive(Default)]
pub(crate) struct NativeSearchPresenter;

impl SearchPresenter for NativeSearchPresenter {
    fn present(
        &mut self,
        window: AnyWindowHandle,
        request: SearchRequest,
    ) -> Result<Presented, PresentError> {
        #[cfg(target_os = "ios")]
        {
            native::present(window, request)
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = (window, request);
            Err(PresentError::WindowUnavailable)
        }
    }
}

/// Where a session is (the SDK gate stays shut until `InputRestored`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Presenting,
    Open,
    /// A selected row opened; the panel is closing for it.
    Closing,
    /// Dismissed or failed: waiting for UIKit to give first responder back.
    Restoring,
}

#[derive(Default)]
pub(crate) struct SearchState {
    pub(crate) session: Option<SearchSession>,
    /// `None` means `NativeSearchPresenter`.
    presenter: Option<Box<dyn SearchPresenter>>,
    next_session: u64,
    /// The magnifier's painted bounds, the popover's anchor.
    pub(crate) anchor: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl std::fmt::Debug for SearchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchState")
            .field("session", &self.session)
            .field("next_session", &self.next_session)
            .finish_non_exhaustive()
    }
}

impl SearchState {
    #[cfg(test)]
    pub(crate) fn set_presenter(&mut self, presenter: Box<dyn SearchPresenter>) {
        self.presenter = Some(presenter);
    }
}

pub(crate) struct SearchSession {
    /// yebisu's own counter: an event of an older session is ignored.
    id: u64,
    pub(crate) phase: Phase,
    /// Dropped at `Dismissed` or `Failed`, which releases the SDK handle.
    control: Option<Box<dyn SearchControl>>,
    /// `terminal.input.retirement_revision()` right after the open's detach.
    pub(crate) revision_at_open: u64,
    pub(crate) presentation: Presentation,
    pub(crate) rows: results::RowTable,
    /// The `(revision, id)` that opened and is waiting for its dismissal.
    pub(crate) pending: Option<(u64, u64)>,
    /// The one FIFO receiver of the session's events.
    _events: gpui::Task<()>,
}

impl std::fmt::Debug for SearchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchSession")
            .field("id", &self.id)
            .field("phase", &self.phase)
            .field("holds_control", &self.control.is_some())
            .field("revision_at_open", &self.revision_at_open)
            .field("presentation", &self.presentation)
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

/// The magnifier's spoken name: with the filter it keeps after closing.
pub(crate) fn search_label(query: &str) -> String {
    if query.trim().is_empty() {
        filter::SEARCH_LABEL.to_owned()
    } else {
        format!("{}, filtered by {query}", filter::SEARCH_LABEL)
    }
}

impl Smoke {
    /// From registration until `InputRestored`, as the SDK gate.
    pub(crate) fn search_open(&self) -> bool {
        self.shell.search.session.is_some()
    }

    /// The magnifier's painted bounds, or a touch-sized square at `at`.
    pub(crate) fn search_anchor(&self, at: gpui::Point<Pixels>) -> Bounds<Pixels> {
        self.shell
            .search
            .anchor
            .get()
            .unwrap_or_else(|| Bounds::new(at, size(px(ANCHOR_FALLBACK), px(ANCHOR_FALLBACK))))
    }

    pub(crate) fn open_sidebar_search(
        &mut self,
        anchor: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_modal() {
            self.terminal.live.status = "Close the sheet first".into();
            cx.notify();
            return;
        }
        if self.find.is_some() {
            self.terminal.live.status = "Close Find first".into();
            cx.notify();
            return;
        }
        if self.search_open() {
            return;
        }
        let presentation = if self.search_compact(window) {
            Presentation::Sheet
        } else {
            Presentation::Popover
        };
        let window_width = f32::from(window.viewport_size().width);
        let query = self.sidebar_filter().to_owned();
        let mut rows = results::RowTable::default();
        let (shown, targets) = results::build(self, &query, &mut rows);
        rows.replace(0, targets);
        let request = SearchRequest {
            anchor,
            presentation,
            title: TITLE.into(),
            cancel_label: CANCEL.into(),
            cancel_accessibility_label: CANCEL_LABEL.into(),
            placeholder: PLACEHOLDER.into(),
            query,
            width: POPOVER_WIDTH.min(window_width - EDGE).max(0.),
            height: POPOVER_HEIGHT,
            results: shown,
        };
        let presented = match self.shell.search.presenter.as_mut() {
            Some(presenter) => presenter.present(window.window_handle(), request),
            None => NativeSearchPresenter.present(window.window_handle(), request),
        };
        let (control, mut events) = match presented {
            Ok(session) => session,
            Err(error) => {
                self.terminal.live.status = error.reason().into();
                cx.notify();
                return;
            }
        };
        // What `hand_keys_to_sheet` gives up, without moving GPUI focus and
        // without a fence: the panel's field owns the keys, and UIKit gives
        // first responder back at `InputRestored`.
        if let Some(core) = &self.terminal.core {
            core.invalidate_input();
        }
        self.actions.palette = false;
        self.actions.consumed.clear();
        self.terminal.input.detach();
        let revision_at_open = self.terminal.input.retirement_revision();
        self.shell.search.next_session += 1;
        let id = self.shell.search.next_session;
        let receiver = cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = std::future::poll_fn(|task| events.poll_next(task)).await {
                if this
                    .update_in(cx, |app, window, cx| {
                        app.on_search_event(id, event, window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.shell.search.session = Some(SearchSession {
            id,
            phase: Phase::Presenting,
            control: Some(control),
            revision_at_open,
            presentation,
            rows,
            pending: None,
            _events: receiver,
        });
        // Registered first: `sync_handoff` now yields no stamp, so a Paste
        // that was being prepared is dropped at its completion check.
        self.handoff.sync(None);
        self.handoff.cancel();
        self.handoff.select_mode = false;
        cx.notify();
    }

    /// The width rule (a sheet below 352 pt usable width); a Simulator QA
    /// build may force it (`YEBISU_QA_SEARCH_COMPACT`).
    fn search_compact(&self, window: &Window) -> bool {
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        if let Some(forced) = super::search_qa::forced_compact() {
            return forced;
        }
        self.compact_window_at(window)
    }

    pub(crate) fn on_search_event(
        &mut self,
        id: u64,
        event: SearchEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.search.session.as_ref().map(|s| s.id) != Some(id) {
            return;
        }
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        super::search_qa::event(self, &event);
        match event {
            SearchEvent::Opened => self.with_search(|session| session.phase = Phase::Open),
            SearchEvent::Failed(reason) => {
                self.with_search(|session| {
                    session.control = None;
                    session.phase = Phase::Restoring;
                });
                self.terminal.live.status = format!("Search could not open ({reason:?})");
            }
            SearchEvent::QueryChanged(query) => {
                self.set_sidebar_filter(&query);
                self.show_search_results(None);
            }
            SearchEvent::ClearRequested => {
                self.clear_sidebar_filter();
                if let Some(control) = self.search_control() {
                    let _ = control.set_query("");
                }
                self.show_search_results(None);
            }
            SearchEvent::Selected { id, revision } => self.open_search_row(revision, id, cx),
            SearchEvent::Dismissed(reason) => {
                if reason == DismissReason::Escape {
                    self.clear_sidebar_filter();
                }
                self.with_search(|session| {
                    session.control = None;
                    session.phase = Phase::Restoring;
                });
            }
            SearchEvent::InputRestored(restored) => {
                let Some(session) = self.shell.search.session.take() else {
                    return;
                };
                if restored == Restored::Text {
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

    fn with_search(&mut self, change: impl FnOnce(&mut SearchSession)) {
        if let Some(session) = self.shell.search.session.as_mut() {
            change(session);
        }
    }

    fn search_control(&self) -> Option<&dyn SearchControl> {
        self.shell.search.session.as_ref()?.control.as_deref()
    }

    /// Rebuilds the results for the current filter and shows them, keeping
    /// the rows of the list just replaced (a tap on it may already be queued).
    fn show_search_results(&mut self, message: Option<&str>) {
        let query = self.sidebar_filter().to_owned();
        let Some(mut rows) = self
            .shell
            .search
            .session
            .as_mut()
            .map(|session| std::mem::take(&mut session.rows))
        else {
            return;
        };
        let (mut shown, targets) = results::build(self, &query, &mut rows);
        shown.message = message.map(str::to_owned);
        let revision = self.search_control().map(|control| control.update(shown));
        if let Some(Ok(revision)) = revision {
            rows.replace(revision, targets);
        } else if let Some(Err(error)) = revision {
            tracing::warn!(?error, "search results not shown");
        }
        self.with_search(|session| session.rows = rows);
    }

    /// A row of the panel was tapped: open it as the sidebar would, then let
    /// the panel close; a refusal keeps the panel up with a message.
    fn open_search_row(&mut self, revision: u64, id: u64, cx: &mut Context<Self>) {
        let Some(target) = self
            .shell
            .search
            .session
            .as_ref()
            .and_then(|session| session.rows.lookup(revision, id))
            .cloned()
        else {
            tracing::warn!(revision, id, "search row not in the shown results");
            self.observation.record(serde_json::json!({
                "kind": "search-stale-row",
                "revision": revision,
                "id": id,
            }));
            return;
        };
        let opened = match target {
            results::RowTarget::Herdr1(tap) => self.sidebar_tap(tap),
            results::RowTarget::Herdr2 {
                host,
                boot,
                generation,
                row,
            } => self.open_herdr2_row(&host, &boot, generation, row.core(), cx),
        };
        match opened {
            Ok(()) => {
                self.with_search(|session| {
                    session.pending = Some((revision, id));
                    session.phase = Phase::Closing;
                });
                // The control stays held until `Dismissed(Selected(id))`, so
                // the old terminal never takes input in between.
                if let Some(Err(error)) = self.search_control().map(|c| c.dismiss_selected(id)) {
                    tracing::warn!(?error, "search panel not dismissed after a selection");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "search row did not open");
                self.show_search_results(Some(UNAVAILABLE));
            }
        }
    }
}

/// The SDK's session behind the mirror types (iOS only).
#[cfg(target_os = "ios")]
mod native {
    use super::{
        DismissReason, FailReason, PresentError, Presentation, Presented, Restored, SearchControl,
        SearchEvent, SearchEvents, SearchRequest, SearchResults,
    };
    use futures::Stream;
    use gpui::AnyWindowHandle;
    use herdr_uikit::{
        NativeSearchDismissReason as Dismiss, NativeSearchEmpty, NativeSearchError,
        NativeSearchEvent, NativeSearchEvents, NativeSearchFailure, NativeSearchHandle,
        NativeSearchItem, NativeSearchPresentation, NativeSearchRequest, NativeSearchResults,
        NativeSearchSection, NativeSearchStyle, RestoredResponder,
    };
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    pub(super) fn present(
        window: AnyWindowHandle,
        request: SearchRequest,
    ) -> Result<Presented, PresentError> {
        let request = NativeSearchRequest {
            anchor: request.anchor,
            presentation: match request.presentation {
                Presentation::Popover => NativeSearchPresentation::Popover,
                Presentation::Sheet => NativeSearchPresentation::Sheet,
            },
            title: request.title,
            cancel_label: request.cancel_label,
            cancel_accessibility_label: request.cancel_accessibility_label,
            placeholder: request.placeholder,
            query: request.query,
            width: request.width,
            height: request.height,
            style: NativeSearchStyle::default(),
            results: results(request.results),
        };
        let (handle, events) =
            herdr_uikit::present_native_search(window, request).map_err(error)?;
        Ok((Box::new(Control(handle)), Box::new(Events(events))))
    }

    struct Control(NativeSearchHandle);

    impl SearchControl for Control {
        fn update(&self, shown: SearchResults) -> Result<u64, PresentError> {
            self.0.update(results(shown)).map_err(error)
        }
        fn dismiss_selected(&self, id: u64) -> Result<(), PresentError> {
            self.0.dismiss_selected(id).map_err(error)
        }
        fn set_query(&self, query: &str) -> Result<(), PresentError> {
            self.0.set_query(query).map_err(error)
        }
    }

    struct Events(NativeSearchEvents);

    impl SearchEvents for Events {
        fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<SearchEvent>> {
            Pin::new(&mut self.0)
                .poll_next(cx)
                .map(|event| event.map(convert))
        }
    }

    fn results(shown: SearchResults) -> NativeSearchResults {
        NativeSearchResults {
            status: shown.status,
            message: shown.message,
            sections: shown
                .sections
                .into_iter()
                .map(|section| NativeSearchSection {
                    title: section.title,
                    items: section
                        .items
                        .into_iter()
                        .map(|item| NativeSearchItem {
                            id: item.id,
                            title: item.title,
                            detail: item.detail,
                            dot: item.dot.map(gpui::rgb),
                            accessibility_label: item.accessibility_label,
                        })
                        .collect(),
                })
                .collect(),
            empty: shown.empty.map(|empty| NativeSearchEmpty {
                message: empty.message,
                clear_label: empty.clear_label,
            }),
        }
    }

    fn error(error: NativeSearchError) -> PresentError {
        match error {
            NativeSearchError::NotMainThread => PresentError::NotMainThread,
            NativeSearchError::WindowUnavailable => PresentError::WindowUnavailable,
            NativeSearchError::AlreadyPresented => PresentError::AlreadyPresented,
            NativeSearchError::RetirementInProgress => PresentError::RetirementInProgress,
            NativeSearchError::Ended => PresentError::Ended,
        }
    }

    fn convert(event: NativeSearchEvent) -> SearchEvent {
        match event {
            NativeSearchEvent::Opened => SearchEvent::Opened,
            NativeSearchEvent::QueryChanged(query) => SearchEvent::QueryChanged(query),
            NativeSearchEvent::ClearRequested => SearchEvent::ClearRequested,
            NativeSearchEvent::Selected { id, revision } => SearchEvent::Selected { id, revision },
            NativeSearchEvent::Dismissed(reason) => SearchEvent::Dismissed(match reason {
                Dismiss::Cancel => DismissReason::Cancel,
                Dismiss::Escape => DismissReason::Escape,
                Dismiss::OutsideTap => DismissReason::OutsideTap,
                Dismiss::Requested => DismissReason::Requested,
                Dismiss::Return => DismissReason::Return,
                Dismiss::Selected(id) => DismissReason::Selected(id),
                Dismiss::SwipedDown => DismissReason::SwipedDown,
                Dismiss::System => DismissReason::System,
                Dismiss::WindowClosed => DismissReason::WindowClosed,
            }),
            NativeSearchEvent::Failed(failure) => SearchEvent::Failed(match failure {
                NativeSearchFailure::FieldRefused => FailReason::FieldRefused,
                NativeSearchFailure::PresenterBusy => FailReason::PresenterBusy,
                NativeSearchFailure::WindowDetached => FailReason::WindowDetached,
            }),
            NativeSearchEvent::InputRestored(restored) => {
                SearchEvent::InputRestored(match restored {
                    RestoredResponder::Text => Restored::Text,
                    RestoredResponder::Drawing => Restored::Drawing,
                    RestoredResponder::WindowClosed => Restored::WindowClosed,
                    RestoredResponder::None => Restored::None,
                })
            }
        }
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
pub(crate) mod tests;
