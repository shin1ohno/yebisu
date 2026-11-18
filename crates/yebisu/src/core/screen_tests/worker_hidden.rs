//! With `integrator-patches/02-core-worker-hidden-size.patch` the worker sends
//! through `pane_sizes` with the active pane: a pane hidden by the one-pane
//! rule gets no resize (plan v2 6.3). Without the patch the worker calls
//! `pane_size`, which always asks for pane 0, so this test fails on the
//! unpatched branch.
use crate::{
    app::Smoke,
    core::{Command, screen::probe},
};
use gpui::{Bounds, TestAppContext, point, px, size};
use sheprd_backend::Size;
use sheprd_client::{ConnectOptions, protocol::ClientSurfaceSize};
use std::time::{Duration, Instant};

struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        crate::shell::single_pane::Mailbox::default().publish(None);
    }
}

#[gpui::test]
fn hidden_pane_keeps_its_size_through_the_worker(cx: &mut TestAppContext) {
    let _reset = Reset;
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (backend, _wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.connect_core_using(cx, move |_| Ok(backend))
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let split = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.connected
                && this.terminal.live.status.contains("1 pane(s)")
                && this
                    .terminal
                    .core
                    .as_ref()
                    .is_some_and(|core| core.send(Command::Split).is_ok())
        });
        if split {
            break;
        }
        assert!(Instant::now() < deadline, "core never connected");
        std::thread::sleep(Duration::from_millis(5));
    }
    let whole = Size::new(53, 29).unwrap_or_else(|e| panic!("size: {e}"));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.geometry.grid =
                Bounds::new(point(px(0.0), px(0.0)), size(px(530.0), px(600.0)));
            this.terminal.geometry.cell_width = 10.0;
            this.terminal.geometry.cell_height = 20.0;
            this.enforce_pane_mode(window, cx);
            this.terminal.resize_if_ready(ConnectOptions {
                surface_size: ClientSurfaceSize { cols: 53, rows: 29 },
                ..Default::default()
            });
        })
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // Pane 1 is active after the split and drawn; pane 0 is hidden.
        if probe::calls_for(whole, 2)
            .iter()
            .any(|(active, sent)| *active == 1 && *sent == [None, Some(whole)])
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the worker resized the hidden pane: {:?}",
            probe::calls_for(whole, 2)
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
