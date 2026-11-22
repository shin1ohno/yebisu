use super::super::tests::assert_terminal_origin;
use super::{Page, drag_page};
use crate::{app::Smoke, core::Command, mock_peer};
use gpui::{
    Entity, Pixels, Point, TestAppContext, TouchDragEvent, TouchEvent, TouchId, TouchPhase,
    VisualTestContext, point, px, size,
};
use sheprd_client::protocol::ClientMessage;
use std::time::{Duration, Instant};

fn touch(cx: &mut VisualTestContext, phase: TouchPhase, position: Point<Pixels>) {
    cx.simulate_event(TouchEvent {
        id: TouchId(1),
        phase,
        position,
        predicted_position: None,
        force: None,
    });
}

fn wait_for(view: &Entity<Smoke>, cx: &mut VisualTestContext, ready: impl Fn(&Smoke) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        ready(this)
    }) {
        assert!(Instant::now() < deadline, "expected pane projection");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
}

#[test]
fn pager_ignores_vertical_and_cancelled_drags_and_wraps_at_the_edges() {
    let gesture = |phase, x, y| TouchDragEvent {
        phase,
        start_position: point(px(0.0), px(0.0)),
        position: point(px(x), px(y)),
    };
    assert_eq!(
        drag_page(&gesture(TouchPhase::Ended, -48.0, 5.0), None),
        Some(Page::Next),
    );
    assert_eq!(
        drag_page(&gesture(TouchPhase::Ended, 48.0, 5.0), None),
        Some(Page::Previous),
    );
    assert_eq!(
        drag_page(&gesture(TouchPhase::Ended, -5.0, 48.0), None),
        None,
    );
    assert_eq!(
        drag_page(&gesture(TouchPhase::Cancelled, -48.0, 0.0), None),
        None,
    );
    assert_eq!(
        drag_page(&gesture(TouchPhase::Ended, 0.0, 0.0), Some(Page::At(1))),
        Some(Page::At(1)),
    );
    assert_eq!(Page::Previous.index(0, 2), Some(1));
    assert_eq!(Page::Next.index(0, 2), Some(1));
    assert_eq!(Page::Next.index(1, 2), Some(0));
    assert_eq!(Page::At(2).index(0, 2), None);
}

#[gpui::test]
fn herdr1_dot_tap_focuses_once_when_click_and_drag_end_are_delivered(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts, release) =
        mock_peer::connect_controlled().unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.shell.fit.insets = Some(gpui::WindowInsets {
            ime: gpui::Edges {
                bottom: px(498.0),
                ..gpui::Edges::default()
            },
            ..gpui::WindowInsets::default()
        });
        cx.notify();
    });
    cx.simulate_resize(size(px(1376.0), px(1032.0)));
    wait_for(&view, cx, |this| this.pager().is_some());
    let (shown, status) = view.read_with(cx, |this, _| {
        (
            this.pager().unwrap_or_else(|| panic!("pager")),
            this.terminal.live.status.clone(),
        )
    });
    let target = 1 - shown.panes.focused;
    let at = cx
        .debug_bounds(if target == 0 {
            "shell-pane-dot-0"
        } else {
            "shell-pane-dot-1"
        })
        .unwrap_or_else(|| panic!("other dot"))
        .center();

    touch(cx, TouchPhase::Started, at);
    touch(cx, TouchPhase::Ended, at);
    assert!(view.read_with(cx, |this, _| this.terminal.navigating.is_some()));
    // Deliberately replay the same tap through the pointer click path before
    // the peer can acknowledge the finger gesture and move the focus.
    cx.simulate_click(at, Default::default());
    assert_eq!(
        view.read_with(cx, |this, _| this.terminal.live.status.clone()),
        status
    );

    let mut requests = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Ok(message) =
        receipts.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    {
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = message {
            requests.push(
                serde_json::from_str::<serde_json::Value>(&request)
                    .unwrap_or_else(|error| panic!("request: {error}")),
            );
            break;
        }
    }
    assert_eq!(requests.len(), 1);
    release
        .send(())
        .unwrap_or_else(|error| panic!("release focus: {error}"));
    wait_for(&view, cx, |this| {
        this.pager()
            .is_some_and(|pager| pager.panes.focused == target)
    });
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(150)) {
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = message {
            let request = serde_json::from_str::<serde_json::Value>(&request)
                .unwrap_or_else(|error| panic!("request: {error}"));
            requests.push(request);
        }
    }
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0]["method"], "pane.focus");
    assert_eq!(requests[0]["params"]["pane_id"], shown.panes.ids[target]);
}

#[gpui::test]
fn herdr2_dot_swipe_focuses_the_other_pane(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(320.0), px(834.0)));
    let (backend, _receipts) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("core peer: {error}"));
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait_for(&view, cx, |this| this.terminal.coherent_frame().is_some());
    view.update(cx, |this, _| this.core_command(Command::Split));
    wait_for(&view, cx, |this| {
        this.pager().is_some_and(|pager| pager.core)
    });
    let shown = view.read_with(cx, |this, _| {
        this.pager().unwrap_or_else(|| panic!("pager"))
    });
    assert_terminal_origin(&view, cx, 44.0);
    let at = cx
        .debug_bounds("shell-pane-dot-1")
        .unwrap_or_else(|| panic!("second dot"))
        .center();
    let end = at - point(px(60.0), px(0.0));
    touch(cx, TouchPhase::Started, at);
    touch(cx, TouchPhase::Moved, end);
    touch(cx, TouchPhase::Ended, end);
    let target = &shown.panes.ids[1 - shown.panes.focused];
    wait_for(&view, cx, |this| {
        this.terminal
            .coherent_frame()
            .is_some_and(|(snapshot, _)| snapshot.focused_pane_id.as_ref() == Some(target))
    });
    view.update(cx, |this, cx| {
        let snapshot = this
            .terminal
            .live
            .snapshot
            .as_mut()
            .unwrap_or_else(|| panic!("projection"));
        std::sync::Arc::make_mut(snapshot).focused_pane_id =
            Some(shown.panes.ids[shown.panes.focused].clone());
        assert!(this.terminal.coherent_frame().is_some());
        assert!(
            this.core_tabs()
                .iter()
                .any(|tab| tab.active && tab.panes.len() == 2)
        );
        cx.notify();
    });
    assert_terminal_origin(&view, cx, 0.0);
}
