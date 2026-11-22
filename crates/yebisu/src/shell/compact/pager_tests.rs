use super::{Page, drag_page};
use gpui::{TouchDragEvent, TouchPhase, point, px};

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
fn paging_herdr1_sends_focus_without_a_zoom_request(cx: &mut gpui::TestAppContext) {
    use crate::{app::Smoke, mock_peer};
    use sheprd_client::protocol::ClientMessage;
    use std::time::{Duration, Instant};

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
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
    });
    cx.simulate_resize(gpui::size(px(1376.0), px(1032.0)));
    let deadline = Instant::now() + Duration::from_secs(10);
    let shown = loop {
        if let Some(pager) = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.pager()
        }) {
            break pager;
        }
        assert!(Instant::now() < deadline, "two coherent panes");
        std::thread::sleep(Duration::from_millis(5));
    };
    let target = 1 - shown.panes.focused;
    view.update_in(cx, |this, window, cx| {
        this.turn_pane_page(&shown, Page::At(target), window, cx);
    });
    let mut requests = Vec::new();
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
