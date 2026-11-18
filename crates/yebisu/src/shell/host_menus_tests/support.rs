//! Shared fixtures for the Host / Space menu tests.
use crate::{app::Smoke, live::Live, mock_peer, shell::ChromeState, shell::HerdrKind};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use sheprd_client::ClientEvent;
use std::sync::Arc;

/// The five chrome states of plan v2 5.1 rule 2.
pub(super) fn states() -> Vec<ChromeState> {
    let online = ChromeState {
        connected: true,
        ..ChromeState::default()
    };
    vec![
        online,
        ChromeState::default(),
        ChromeState {
            kind: HerdrKind::Herdr2,
            ..online
        },
        ChromeState {
            selecting: true,
            ..online
        },
        ChromeState {
            sheet_open: true,
            ..online
        },
    ]
}

/// A Smoke whose selected host is `host`, connected to a mock Herdr 1
/// snapshot, in an active window.
pub(super) fn smoke<'a>(
    cx: &'a mut TestAppContext,
    host: &str,
) -> (Entity<Smoke>, &'a mut VisualTestContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    let host = host.to_owned();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let mut live = Live::default();
            live.connected = true;
            live.methods = vec!["tab.create".into(), "tab.focus".into()];
            live.apply(ClientEvent::Snapshot(Arc::new(
                serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}")),
            )));
            this.terminal.live = live;
            this.terminal.open = true;
            this.terminal.reconnect.target = host;
            this.terminal_focus.focus(window, cx);
        })
    });
    (view, cx)
}
