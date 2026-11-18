//! Review cf8bbcccc2164abe F1 (L2b patch 02): with the Add host sheet open
//! over a connected terminal, typed text goes into the sheet's host field and
//! never to the terminal or the remote shell. Keys and text go through
//! GPUI's real dispatch (`simulate_keystrokes`).
use crate::{
    app::Smoke,
    mock_peer,
    shell::{add_host::SheetMode, footer::FooterItem},
};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

fn connected(
    cx: &mut TestAppContext,
) -> (
    Entity<Smoke>,
    &mut VisualTestContext,
    mpsc::Receiver<ClientMessage>,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some() && this.handoff.stamp.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx, wire)
}

fn pane_input(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &mpsc::Receiver<ClientMessage>,
) -> Vec<ClientPaneInputEvent> {
    view.update(cx, |this, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.flush_terminal();
    });
    cx.run_until_parked();
    let mut out = Vec::new();
    while let Ok(message) = wire.recv_timeout(Duration::from_millis(300)) {
        if let ClientMessage::ClientShellPaneInput { events, .. } = message {
            out.extend(events);
        }
    }
    out
}

#[gpui::test]
fn add_host_field_takes_typed_text_over_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.footer_tap(FooterItem::Hosts, window, cx)
        })
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.hosts_open_add(SheetMode::Add, "", cx);
            // The user taps the host field (hosts_sheet/view.rs).
            this.focus.focus(window, cx);
        })
    });
    cx.run_until_parked();
    pane_input(&view, cx, &wire);
    cx.simulate_keystrokes("q a");
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(this.hosts_sheet_modal());
        assert!(!this.terminal_owns_text_input());
        assert_eq!(
            this.setup_input.text, "qa",
            "the host field missed the text"
        );
        assert!(this.terminal.input.composition.text.is_empty());
    });
    let sent = pane_input(&view, cx, &wire);
    assert!(
        sent.is_empty(),
        "typed text reached the remote shell: {sent:?}"
    );
}

/// A6 item 3 (review ab783fbc2a944abc S2): `hosts_close` itself gives the
/// terminal its focus back on the next frame, whatever closed the sheet.
#[gpui::test]
fn hosts_close_returns_terminal_focus(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.footer_tap(FooterItem::Hosts, window, cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(!this.terminal_focus.is_focused(window));
            this.hosts_close(cx);
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(
                this.terminal_focus.is_focused(window),
                "hosts_close left the terminal without focus"
            );
        })
    });
}
