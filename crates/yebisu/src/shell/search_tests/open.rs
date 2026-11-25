//! Opening the panel: the width rule, the refusals, the present error, and
//! what the terminal gives up first (issue AC1, AC2, AC12, AC13, AC23).
use super::*;
use crate::shell::search::{PresentError, Presentation};

fn open_at(width: f32, cx: &mut TestAppContext) -> Presentation {
    let (view, _receipts, cx) = terminal_at(cx, "A", width);
    let fake = install_fake(&view, cx);
    view.update_in(cx, |this, window, cx| {
        let anchor = this.search_anchor(point(px(10.), px(10.)));
        this.open_sidebar_search(anchor, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(fake.presents(), 1, "width {width}: nothing was presented");
    fake.request().presentation
}

/// Below 352 pt of usable width the panel is a page sheet, at 352 pt and up a
/// popover (the host has no safe area).
#[gpui::test]
fn open_presentation_follows_the_compact_width(cx: &mut TestAppContext) {
    assert_eq!(open_at(351., cx), Presentation::Sheet);
    assert_eq!(open_at(352., cx), Presentation::Popover);
    assert_eq!(open_at(1200., cx), Presentation::Popover);
}

#[gpui::test]
fn open_request_carries_the_title_query_and_anchor(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    view.update(cx, |this, cx| {
        this.set_sidebar_filter("repo");
        cx.notify();
    });
    cx.run_until_parked();
    opened(&view, &fake, cx);
    let request = fake.request();
    let magnifier = cx
        .debug_bounds("sidebar-search")
        .unwrap_or_else(|| panic!("magnifier"));
    assert_eq!(
        request.anchor, magnifier,
        "the popover is not anchored at the magnifier"
    );
    assert_eq!(request.title, "Search spaces and agents");
    assert_eq!(request.cancel_label, "Cancel");
    assert_eq!(request.cancel_accessibility_label, "Cancel search");
    assert_eq!(request.placeholder, "Name or host");
    assert_eq!(request.query, "repo", "the kept filter is the field's text");
    assert_eq!((request.width, request.height), (360., 560.));
    assert_eq!(request.presentation, Presentation::Popover);
}

/// The compact layout's panel header has the magnifier too.
#[gpui::test]
fn open_from_the_compact_header(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal_at(cx, "A", 340.);
    let fake = install_fake(&view, cx);
    let menu = cx
        .debug_bounds("shell-compact-menu")
        .unwrap_or_else(|| panic!("fixture: the compact menu button"));
    cx.simulate_click(menu.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("shell-compact-sidebar").is_some(),
        "fixture: the compact sidebar panel is open"
    );
    opened(&view, &fake, cx);
    assert_eq!(fake.request().presentation, Presentation::Sheet);
}

fn refused(
    view: &Entity<Smoke>,
    fake: &RecordedSearch,
    cx: &mut VisualTestContext,
    name: &str,
    reason: &str,
) {
    view.update(cx, |this, _| this.terminal.live.status.clear());
    view.update_in(cx, |this, window, cx| {
        this.open_sidebar_search(this.search_anchor(point(px(1.), px(1.))), window, cx);
    });
    cx.run_until_parked();
    assert_eq!(fake.presents(), 0, "{name}: the panel opened");
    assert!(
        phase(view, cx).is_none(),
        "{name}: a session was registered"
    );
    assert_eq!(
        status(view, cx),
        reason,
        "{name}: no reason in the status line"
    );
}

/// Over Settings, Hosts or New space the panel does not open, and the status
/// line says why.
#[gpui::test]
fn open_refused_over_sheets(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    view.update(cx, |this, _| this.settings_modal = true);
    refused(&view, &fake, cx, "settings", "Close the sheet first");
    view.update(cx, |this, _| {
        this.settings_modal = false;
        this.shell.hosts_sheet.open_list();
        assert!(
            this.hosts_sheet_modal(),
            "fixture: Hosts covers the terminal"
        );
    });
    refused(&view, &fake, cx, "hosts", "Close the sheet first");
    view.update(cx, |this, cx| {
        this.shell.hosts_sheet = Default::default();
        this.open_new_space(cx);
        assert!(this.new_space_open(), "fixture: New space is open");
    });
    refused(&view, &fake, cx, "new space", "Close the sheet first");
}

/// Find has its own GPUI field: the panel does not open over it.
#[gpui::test]
fn open_refused_over_find(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, _receipts) =
        crate::find::peer::connect_quiet(false).unwrap_or_else(|e| panic!("find peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("F");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    wait(
        &view,
        cx,
        "the Find-capable peer did not become ready",
        |this| crate::find_view::available(&this.terminal),
    );
    let fake = install_fake(&view, cx);
    view.update_in(cx, |this, window, cx| this.open_find(window, cx));
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |this, _| this.find.is_some()),
        "fixture: Find is open"
    );
    refused(&view, &fake, cx, "find", "Close Find first");
}

/// A second tap while the panel is up presents nothing more.
#[gpui::test]
fn second_tap_does_not_present_again(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    tap_magnifier(cx);
    assert_eq!(fake.presents(), 1, "a second panel was presented");
    assert_eq!(phase(&view, cx), Some(Phase::Open));
}

/// The SDK refused the presentation: no session, the reason in the status
/// line, and the terminal takes keys as before.
#[gpui::test]
fn present_error_leaves_no_session(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    fake.0.borrow_mut().refuse = Some(PresentError::RetirementInProgress);
    tap_magnifier(cx);
    assert_eq!(fake.presents(), 1, "fixture: present was called");
    assert!(
        phase(&view, cx).is_none(),
        "a refused present left a session"
    );
    assert_eq!(
        status(&view, cx),
        PresentError::RetirementInProgress.reason()
    );
    typed_once(&view, &receipts, cx, "w1:p1", "x");
}

/// A composition the terminal held when the panel opened is discarded, not
/// sent; the next key after the panel closed reaches the terminal once.
#[gpui::test]
fn composition_discarded_at_open(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    mark_text(&view, cx, "にほ");
    assert!(
        view.read_with(cx, |this, _| !this
            .terminal
            .input
            .composition
            .text
            .is_empty()),
        "fixture: the terminal holds a composition"
    );
    opened(&view, &fake, cx);
    assert!(
        view.read_with(cx, |this, _| this
            .terminal
            .input
            .composition
            .text
            .is_empty()),
        "the composition survived the open"
    );
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "the composition was sent"
    );
    close(
        &fake,
        cx,
        super::super::DismissReason::Return,
        super::super::Restored::Text,
    );
    typed_once(&view, &receipts, cx, "w1:p1", "z");
}

type Gate = std::sync::Arc<std::sync::Mutex<(bool, Option<std::task::Waker>)>>;

/// Holds a Paste's completion until `release`.
fn completion_gate(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Gate {
    let gate: Gate = Default::default();
    let waiting = gate.clone();
    let task = cx
        .background_executor
        .spawn(std::future::poll_fn(move |cx| {
            let mut g = waiting.lock().unwrap_or_else(|e| e.into_inner());
            if g.0 {
                std::task::Poll::Ready(())
            } else {
                g.1 = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        }));
    view.update(cx, |this, _| this.handoff.completion_gate = Some(task));
    gate
}

fn release(gate: &Gate) {
    let mut g = gate.lock().unwrap_or_else(|e| e.into_inner());
    g.0 = true;
    if let Some(waker) = g.1.take() {
        waker.wake();
    }
}

fn start_paste(view: &Entity<Smoke>, cx: &mut VisualTestContext, text: &str) {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.into()));
    let view = view.clone();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.start_paste(false, cx);
        });
    });
    cx.run_until_parked();
}

fn pastes(events: &[String]) -> usize {
    events.iter().filter(|e| e.contains("Paste(")).count()
}

/// A Paste prepared before the panel opened is dropped at its completion
/// (Herdr 1; Herdr 2 is a cancel source in `core::image_tests`).
#[gpui::test]
fn paste_prepared_before_open_dropped(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    let gate = completion_gate(&view, cx);
    start_paste(&view, cx, "prepared-before-open");
    assert_eq!(
        view.read_with(cx, |this, _| this.handoff.workers),
        1,
        "fixture: the Paste is being prepared"
    );
    let serial = view.read_with(cx, |this, _| this.handoff.serial);
    opened(&view, &fake, cx);
    assert_ne!(
        view.read_with(cx, |this, _| this.handoff.serial),
        serial,
        "the open did not cancel the Paste"
    );
    assert!(view.read_with(cx, |this, _| this.handoff.stamp.is_none()));
    release(&gate);
    wait(&view, cx, "the Paste worker did not finish", |this| {
        this.handoff.workers == 0
    });
    assert_eq!(
        pastes(&sent(&view, &receipts, cx)),
        0,
        "the prepared Paste was sent"
    );
}

/// A Paste started while the panel is up finds no stamp and sends nothing.
#[gpui::test]
fn paste_started_while_open_sends_nothing(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    start_paste(&view, cx, "while-open");
    assert_eq!(view.read_with(cx, |this, _| this.handoff.workers), 0);
    assert_eq!(
        pastes(&sent(&view, &receipts, cx)),
        0,
        "a Paste reached the terminal"
    );
}

/// A Paste that went out before the panel opened arrives once, and the open
/// sends it again neither then nor after the restore.
#[gpui::test]
fn paste_sent_before_open_once(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    start_paste(&view, cx, "sent-before-open");
    wait(&view, cx, "the Paste worker did not finish", |this| {
        this.handoff.workers == 0
    });
    let before = sent(&view, &receipts, cx);
    assert_eq!(
        pastes(&before),
        1,
        "fixture: the Paste was sent: {before:?}"
    );
    opened(&view, &fake, cx);
    close(
        &fake,
        cx,
        super::super::DismissReason::Return,
        super::super::Restored::Text,
    );
    assert_eq!(
        pastes(&sent(&view, &receipts, cx)),
        0,
        "the Paste was sent again"
    );
}
