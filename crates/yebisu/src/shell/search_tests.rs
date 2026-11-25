//! Sidebar search (R24, issue #138): host tests with a recorded presenter.
//!
//! Every test that guards a path while the panel is up first calls
//! `opened`: the fake presented exactly once and the session is `Open`
//! after `Opened`. A regression that never opens fails there, not at the
//! guarded step (issue AC24).
use super::{Phase, SearchEvent};
use crate::{app::Smoke, mock_peer};
use gpui::{
    Entity, EntityInputHandler, Modifiers, TestAppContext, VisualTestContext, point, px, size,
};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

#[path = "search_tests/band.rs"]
mod band;
#[path = "search_tests/display.rs"]
mod display;
#[path = "search_tests/events.rs"]
mod events;
#[path = "search_tests/fake.rs"]
pub(crate) mod fake;
#[path = "search_tests/open.rs"]
mod open;
#[path = "search_tests/select.rs"]
mod select;
#[path = "search_tests/terminal.rs"]
mod terminal;

pub(crate) use fake::RecordedSearch;

pub(crate) type Receipts = Receiver<ClientMessage>;

/// Waits for `ready`, polling the terminal, at most 5 s.
pub(crate) fn wait(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    what: &str,
    mut ready: impl FnMut(&mut Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        ready(this)
    }) {
        assert!(Instant::now() < deadline, "fixture: {what}");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
}

/// A Herdr 1 terminal on the private peer for `host` (spaces `w1`, `w2`,
/// panes `w1:p1`, `w2:p2`), drawn at `width`×1000 with the sidebar open.
pub(crate) fn terminal_at<'a>(
    cx: &'a mut TestAppContext,
    host: &str,
    width: f32,
) -> (Entity<Smoke>, Receipts, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(width), px(1000.)));
    let receipts = connect(&view, cx, host);
    (view, receipts, cx)
}

pub(crate) fn terminal(
    cx: &mut TestAppContext,
) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
    terminal_at(cx, "A", 1200.)
}

/// Connects `host` on its own recorded peer and selects it.
pub(crate) fn connect(view: &Entity<Smoke>, cx: &mut VisualTestContext, host: &str) -> Receipts {
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host(host);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    wait(view, cx, "private peer did not become ready", |this| {
        this.terminal.live.surface.is_some()
            && !this.row_tabs().is_empty()
            && crate::board::snapshot(&this.terminal.live).is_some()
    });
    sent(view, &receipts, cx);
    receipts
}

/// The pane input the terminal sent since the last call, as
/// `pane: event` strings.
pub(crate) fn sent(
    view: &Entity<Smoke>,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
) -> Vec<String> {
    for _ in 0..10 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut events = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let ClientMessage::ClientShellPaneInput {
            pane_id,
            events: input,
        } = message
        {
            events.extend(
                input
                    .iter()
                    .map(|event: &ClientPaneInputEvent| format!("{pane_id}: {event:?}")),
            );
        }
    }
    events
}

/// Every endpoint request (navigation) the peer received since the last
/// call, as `method params` strings.
pub(crate) fn requests(receipts: &Receipts) -> Vec<String> {
    let mut seen = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = message {
            let value: serde_json::Value =
                serde_json::from_str(&request).unwrap_or_else(|e| panic!("request: {e}"));
            seen.push(format!("{} {}", value["method"], value["params"]));
        }
    }
    seen
}

pub(crate) fn install_fake(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> RecordedSearch {
    let fake = RecordedSearch::default();
    let presenter = fake.clone();
    view.update(cx, |this, _| {
        this.shell.search.set_presenter(Box::new(presenter))
    });
    fake
}

/// Taps the magnifier in the sidebar header.
pub(crate) fn tap_magnifier(cx: &mut VisualTestContext) {
    let at = cx
        .debug_bounds("sidebar-search")
        .unwrap_or_else(|| panic!("the magnifier is not painted"));
    cx.simulate_click(
        point(
            at.origin.x + at.size.width / 2.,
            at.origin.y + at.size.height / 2.,
        ),
        Modifiers::none(),
    );
    cx.run_until_parked();
}

pub(crate) fn push(fake: &RecordedSearch, cx: &mut VisualTestContext, event: SearchEvent) {
    fake.push(event);
    cx.run_until_parked();
}

pub(crate) fn phase(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Option<Phase> {
    view.read_with(cx, |this, _| {
        this.shell.search.session.as_ref().map(|s| s.phase)
    })
}

pub(crate) fn filter(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |this, _| this.sidebar_filter().to_owned())
}

pub(crate) fn status(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |this, _| this.terminal.live.status.clone())
}

/// The precondition of every guarded step: the magnifier presented the
/// panel once and the session is `Open`.
pub(crate) fn opened(view: &Entity<Smoke>, fake: &RecordedSearch, cx: &mut VisualTestContext) {
    let before = fake.presents();
    tap_magnifier(cx);
    assert_eq!(
        fake.presents(),
        before + 1,
        "the magnifier did not present the panel"
    );
    assert_eq!(phase(view, cx), Some(Phase::Presenting));
    push(fake, cx, SearchEvent::Opened);
    assert_eq!(
        phase(view, cx),
        Some(Phase::Open),
        "the session is not open"
    );
}

/// Closes the panel the way UIKit does: the dismissal, then the restore.
pub(crate) fn close(
    fake: &RecordedSearch,
    cx: &mut VisualTestContext,
    reason: super::DismissReason,
    restored: super::Restored,
) {
    push(fake, cx, SearchEvent::Dismissed(reason));
    push(fake, cx, SearchEvent::InputRestored(restored));
}

/// Software keyboard text through the platform text input.
pub(crate) fn type_text(view: &Entity<Smoke>, cx: &mut VisualTestContext, text: &str) {
    let view = view.clone();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_text_in_range(None, text, window, cx)
        });
    });
    cx.run_until_parked();
}

/// A composition the IME holds in the terminal (marked, not committed).
pub(crate) fn mark_text(view: &Entity<Smoke>, cx: &mut VisualTestContext, text: &str) {
    let view = view.clone();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_and_mark_text_in_range(None, text, None, window, cx);
        });
    });
    cx.run_until_parked();
}

/// One key typed after the panel closed reaches `pane` exactly once.
pub(crate) fn typed_once(
    view: &Entity<Smoke>,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
    pane: &str,
    text: &str,
) {
    type_text(view, cx, text);
    let events = sent(view, receipts, cx);
    assert_eq!(
        events,
        [format!("{pane}: TextCommit({text:?})")],
        "the next key did not reach the terminal exactly once"
    );
}

/// The ids of the rows on screen titled `title`.
pub(crate) fn row_id(fake: &RecordedSearch, title: &str) -> (u64, u64) {
    let (revision, results) = fake.shown();
    let id = results
        .sections
        .iter()
        .flat_map(|section| &section.items)
        .find(|item| item.title == title)
        .map(|item| item.id)
        .unwrap_or_else(|| panic!("no row {title:?} in {results:?}"));
    (revision, id)
}
