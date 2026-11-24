//! T10: Herdr 2. The core worker writes the wheel to the terminal of the
//! pane under the finger, focused or not, and never to an ended pane. The
//! mock core echoes input, so typing the mouse-reporting mode makes each
//! pane's emulator report the wheel as bytes on that pane's channel.
use super::*;
use crate::core::Command;

const REPORT_MOUSE: &str = "\x1b[?1000h\x1b[?1006h";

type CoreWire = mpsc::Receiver<(u32, Vec<u8>)>;
type Writes = Vec<(u32, Vec<u8>)>;

fn pump(view: &Entity<Smoke>, cx: &mut VisualTestContext, done: impl Fn(&Smoke) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            done(this)
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "core state: {} {:?} {:?}",
            view.read_with(cx, |this, _| this.terminal.live.status.clone()),
            view.read_with(cx, |this, _| this.core_tabs()),
            view.read_with(cx, |this, _| panes(this))
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn panes(this: &Smoke) -> Vec<String> {
    this.terminal
        .live
        .surface
        .as_ref()
        .map(|surface| surface.panes.iter().map(|p| p.pane_id.clone()).collect())
        .unwrap_or_default()
}

fn focused(this: &Smoke) -> Option<String> {
    this.terminal
        .live
        .snapshot
        .as_ref()?
        .focused_pane_id
        .clone()
}

fn command(view: &Entity<Smoke>, cx: &mut VisualTestContext, command: Command) {
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core session"))
            .send(command)
            .unwrap_or_else(|e| panic!("command: {e}"));
    });
}

/// Types `text` into the focused pane. Returns that pane's channel and
/// every terminal write up to and including the typed text.
fn typed(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &CoreWire,
    text: &str,
) -> (u32, Writes) {
    let destination = view.update(cx, |this, _| {
        crate::terminal_input::destination(&this.terminal.live, this.terminal.link.generation)
            .unwrap_or_else(|| panic!("destination"))
    });
    command(
        view,
        cx,
        Command::Input(destination, ClientPaneInputEvent::TextCommit(text.into())),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    loop {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        if let Ok((channel, bytes)) = wire.recv_timeout(Duration::from_millis(5)) {
            let found = bytes == text.as_bytes();
            seen.push((channel, bytes));
            if found {
                return (channel, seen);
            }
        }
        assert!(Instant::now() < deadline, "{text:?} not written: {seen:?}");
    }
}

fn index(view: &Entity<Smoke>, cx: &mut VisualTestContext, pane: &str) -> usize {
    view.read_with(cx, |this, _| {
        panes(this)
            .iter()
            .position(|id| id == pane)
            .unwrap_or_else(|| panic!("pane {pane}"))
    })
}

fn focus(view: &Entity<Smoke>, cx: &mut VisualTestContext, pane: &str) {
    let index = index(view, cx, pane);
    command(view, cx, Command::Focus(index));
    pump(view, cx, |this| {
        focused(this).as_deref() == Some(pane) && this.terminal.input.ready
    });
}

/// The SGR wheel-up reports written on `channel`.
fn reports(writes: &Writes, channel: u32) -> usize {
    writes
        .iter()
        .filter(|(written, _)| *written == channel)
        .map(|(_, bytes)| String::from_utf8_lossy(bytes).matches("\x1b[<64;").count())
        .sum()
}

fn wheel(view: &Entity<Smoke>, cx: &mut VisualTestContext, pane: &str, rows: f32) {
    let row = cell_height(view, cx);
    let at = over(view, cx, pane);
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: pixels(0., rows * row),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Started,
        source: ScrollSource::Touch,
    });
}

fn start(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> CoreWire {
    let (backend, wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("mock core: {e}"));
    view.update(cx, |this, cx| {
        this.select_host("issue-137-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    pump(view, cx, |this| this.terminal.live.connected);
    wire
}

/// A swipe over a Herdr 2 pane reaches its terminal through the worker.
#[gpui::test]
fn a_swipe_over_a_herdr2_pane_reaches_its_terminal(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let wire = start(&view, cx);
    pump(&view, cx, |this| this.terminal.input.ready);
    let pane = view.read_with(cx, |this, _| {
        focused(this).unwrap_or_else(|| panic!("focus"))
    });
    let (channel, _) = typed(&view, cx, &wire, REPORT_MOUSE);
    typed(&view, cx, &wire, "settled");
    wheel(&view, cx, &pane, 2.);
    let (_, writes) = typed(&view, cx, &wire, "after-wheel");
    assert_eq!(reports(&writes, channel), 2, "{writes:?}");
}

fn wheel_up(lines: u16) -> ClientPaneInputEvent {
    ClientPaneInputEvent::Mouse {
        kind: ClientMouseKind::ScrollUp,
        position: ClientMousePosition::Cell { column: 1, row: 1 },
        geometry: None,
        modifiers: 0,
        lines,
    }
}

/// In a split, the worker writes the wheel to the named pane's terminal,
/// not the focused one, and to no pane that left the layout. Not rendered:
/// a drawn root would queue its own resize into the split's batch
/// (core/regression_tests.rs).
#[gpui::test]
fn the_core_writes_the_wheel_to_the_named_pane_in_a_split(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    let cx = cx.add_empty_window();
    let wire = start(&view, cx);
    command(&view, cx, Command::Split);
    pump(&view, cx, |this| panes(this).len() == 2);
    let ids = view.read_with(cx, |this, _| panes(this));
    let second = view.read_with(cx, |this, _| {
        focused(this).unwrap_or_else(|| panic!("focus"))
    });
    let (second_channel, _) = typed(&view, cx, &wire, REPORT_MOUSE);
    let stale = view.update(cx, |this, _| {
        crate::terminal_input::destination(&this.terminal.live, this.terminal.link.generation)
            .unwrap_or_else(|| panic!("destination"))
    });
    focus(&view, cx, &ids[0]);
    let (first_channel, _) = typed(&view, cx, &wire, REPORT_MOUSE);
    assert_ne!(first_channel, second_channel);
    typed(&view, cx, &wire, "settled");
    let wheel = |view: &Entity<Smoke>, cx: &mut VisualTestContext, pane: &str| {
        let destination = view.update(cx, |this, _| {
            crate::terminal_input::destination(&this.terminal.live, this.terminal.link.generation)
                .unwrap_or_else(|| panic!("destination"))
        });
        command(
            view,
            cx,
            Command::Wheel {
                destination,
                pane: pane.into(),
                event: wheel_up(3),
            },
        );
    };
    wheel(&view, cx, &second);
    let (_, writes) = typed(&view, cx, &wire, "after-wheel");
    assert_eq!(reports(&writes, second_channel), 3, "{writes:?}");
    assert_eq!(reports(&writes, first_channel), 0, "{writes:?}");
    // Aimed at a layout that has changed since: nothing is written.
    command(
        &view,
        cx,
        Command::Wheel {
            destination: stale,
            pane: second.clone(),
            event: wheel_up(3),
        },
    );
    let (_, writes) = typed(&view, cx, &wire, "after-stale");
    assert_eq!(reports(&writes, second_channel), 0, "{writes:?}");

    // The second pane's program ends; its pane closes and gets nothing.
    focus(&view, cx, &second);
    typed(&view, cx, &wire, "exit\r");
    pump(&view, cx, |this| panes(this).len() == 1);
    wheel(&view, cx, &second);
    let (_, writes) = typed(&view, cx, &wire, "after-exit");
    assert!(
        writes.iter().all(|(channel, _)| *channel != second_channel),
        "{writes:?}"
    );
    view.read_with(cx, |this, _| {
        assert!(
            this.terminal.live.connected,
            "{}",
            this.terminal.live.status
        );
    });
}
