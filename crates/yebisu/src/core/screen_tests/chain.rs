//! Rule: a column that crosses the one-pane line reaches the Herdr 2 worker
//! (review B1). `enforce_pane_mode` -> mailbox -> the next paint's Resize ->
//! the worker's `pane_size`, through a real worker thread.
use crate::{
    app::Smoke,
    core::{Command, screen::probe},
    shell::single_pane::column_metrics,
};
use gpui::{Bounds, TestAppContext, point, px, size};
use sheprd_backend::Size;
use sheprd_client::{ConnectOptions, protocol::ClientSurfaceSize};
use std::time::{Duration, Instant};

fn options(cols: u16, rows: u16) -> ConnectOptions {
    ConnectOptions {
        surface_size: ClientSurfaceSize { cols, rows },
        ..Default::default()
    }
}

fn whole(cols: u16, rows: u16) -> Size {
    Size::new(cols, rows).unwrap_or_else(|e| panic!("size: {e}"))
}

/// What the worker computed for `whole` panes `count` last, once `want` holds.
fn wait_for(whole: Size, count: usize, want: impl Fn(Size) -> bool) -> Size {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(last) = probe::sent_for(whole, count).last().copied()
            && want(last)
        {
            return last;
        }
        assert!(
            Instant::now() < deadline,
            "worker never sized {whole:?}: {:?}",
            probe::sent_for(whole, count)
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The measured column, as the paint before the frame leaves it.
fn measured(this: &mut Smoke, width: f32) {
    this.terminal.geometry.grid = Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(600.0)));
    this.terminal.geometry.cell_width = 10.0;
    this.terminal.geometry.cell_height = 20.0;
}

/// Puts the global mailbox back when the test ends, also on a failure.
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        crate::shell::single_pane::Mailbox::default().publish(None);
    }
}

#[gpui::test]
fn column_change_reaches_the_worker_resize(cx: &mut TestAppContext) {
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
            if this.terminal.live.connected && this.terminal.live.status.contains("1 pane(s)") {
                this.terminal
                    .core
                    .as_ref()
                    .is_some_and(|core| core.send(Command::Split).is_ok())
            } else {
                false
            }
        });
        if split {
            break;
        }
        assert!(Instant::now() < deadline, "core never connected");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Each frame is: render (enforce_pane_mode), then paint (resize_if_ready),
    // and paint measures the column the next render publishes.
    let frame = |cx: &mut gpui::VisualTestContext, width: f32, cols: u16| {
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.enforce_pane_mode(window, cx);
                measured(this, width);
                this.terminal.resize_if_ready(options(cols, 29));
            })
        })
    };

    // 510 pt: the paint measures it, the first Resize still sees no column.
    view.update(cx, |this, _| measured(this, 510.0));
    frame(cx, 510.0, 51);
    frame(cx, 510.0, 51);
    let narrow = wait_for(whole(51, 29), 2, |last| last == whole(51, 29));
    assert_eq!(narrow, whole(51, 29), "one pane is sent the whole column");

    // Widen to 820 pt: the paint sends Resize before the new column is
    // published; the frame after it publishes and must send it again.
    view.update(cx, |this, _| {
        measured(this, 820.0);
        this.terminal.resize_if_ready(options(82, 29));
    });
    // The worker has sized for the old column (one pane, 82 columns) ...
    wait_for(whole(82, 29), 2, |last| last == whole(82, 29));
    // ... and only a size that is sent again can reach the new column.
    frame(cx, 820.0, 82);
    frame(cx, 820.0, 82);
    let wide = wait_for(whole(82, 29), 2, |last| last == whole(41, 29));
    assert_eq!(wide, whole(41, 29), "two panes share the widened column");
    assert!(column_metrics().is_some());
}
