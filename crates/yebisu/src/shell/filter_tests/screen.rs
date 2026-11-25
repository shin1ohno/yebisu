//! The magnifier on the real screen (owner decision 2026-11-19,
//! `pr77-filter-no-input-decision.json`): it takes no keyboard focus, a tap
//! on it opens the native search panel (issue #138) without moving focus,
//! and it never changes what the terminal receives. On the host the panel's
//! presenter is the recorded one, so a tap opens a session that stays up.
use crate::{app::Smoke, mock_peer};
use gpui::{
    Entity, EntityInputHandler, FocusHandle, Keystroke, Modifiers, TestAppContext,
    VisualTestContext, point, px, size,
};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

type Receipts = Receiver<ClientMessage>;

/// A Herdr 1 terminal on the private peer (one agent, "reviewer", in the
/// space "repo"), drawn at 1200×1000 with the sidebar list open; `hardware`
/// sets whether a hardware keyboard is known.
fn terminal(
    cx: &mut TestAppContext,
    hardware: bool,
) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("filter-screen");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, cx| {
        this.shell.fit.hardware_keyboard = hardware;
        cx.notify();
    });
    cx.run_until_parked();
    sent(&view, &receipts, cx);
    (view, receipts, cx)
}

/// The pane input events the terminal sent since the last call.
fn sent(view: &Entity<Smoke>, receipts: &Receipts, cx: &mut VisualTestContext) -> Vec<String> {
    for _ in 0..10 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut events = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let ClientMessage::ClientShellPaneInput { events: input, .. } = message {
            events.extend(
                input
                    .iter()
                    .map(|event: &ClientPaneInputEvent| format!("{event:?}")),
            );
        }
    }
    events
}

/// Whether the terminal has the keys (focus handles differ between
/// windows, so two runs compare this, not the handle).
fn terminal_focused(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    let view = view.clone();
    cx.update(|window, cx| view.read(cx).terminal_focus.is_focused(window))
}

fn focused(cx: &mut VisualTestContext) -> Option<FocusHandle> {
    cx.update(|window, cx| window.focused(cx))
}

fn tap_field(cx: &mut VisualTestContext) {
    let at = cx
        .debug_bounds("sidebar-search")
        .unwrap_or_else(|| panic!("the filter field is not painted"));
    cx.simulate_click(
        point(
            at.origin.x + at.size.width / 2.,
            at.origin.y + at.size.height / 2.,
        ),
        Modifiers::none(),
    );
    cx.run_until_parked();
}

/// One input: a hardware key press, or text from the software keyboard
/// through the platform text input.
#[derive(Clone)]
enum Input {
    Key(Keystroke),
    Text(&'static str),
}

fn with(modifiers: Modifiers, key: &str, key_char: Option<&str>) -> Input {
    Input::Key(Keystroke {
        modifiers,
        key: key.into(),
        key_char: key_char.map(Into::into),
    })
}

/// Every kind of input the earlier field routed: printable, Option-composed
/// and dead keys, Control, arrows, Tab, Return, Escape, Backspace, Fn,
/// Command shortcuts (Command-Return carries text), and software keyboard
/// text.
fn inputs() -> Vec<Input> {
    let key = |text: &'static str| {
        Input::Key(
            Keystroke::parse(text)
                .ok()
                .unwrap_or_else(|| panic!("{text}")),
        )
    };
    let alt = Modifiers {
        alt: true,
        ..Modifiers::default()
    };
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    vec![
        key("a"),
        key("shift-b"),
        key("space"),
        with(alt, "e", Some("é")),
        with(alt, "e", None),
        key("ctrl-c"),
        key("up"),
        key("tab"),
        key("backspace"),
        key("enter"),
        key("escape"),
        key("f5"),
        key("cmd-k"),
        with(command, "enter", Some("\n")),
        Input::Text("x"),
    ]
}

fn send(input: &Input, view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    match input.clone() {
        Input::Key(keystroke) => {
            cx.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        Input::Text(text) => {
            let view = view.clone();
            cx.update(|window, cx| {
                view.update(cx, |this, cx| {
                    this.replace_text_in_range(None, text, window, cx)
                });
            });
        }
    }
    cx.run_until_parked();
}

/// What the terminal received for each input, and the focus after it;
/// with `tap`, the field is tapped before every input.
fn run(cx: &mut TestAppContext, hardware: bool, tap: bool) -> Vec<(Vec<String>, bool)> {
    let (view, receipts, cx) = terminal(cx, hardware);
    let fake = crate::shell::search::tests::install_fake(&view, cx);
    let start = focused(cx);
    assert!(
        terminal_focused(&view, cx),
        "fixture: the terminal has the keys"
    );
    let mut seen = Vec::new();
    for input in inputs() {
        if tap {
            tap_field(cx);
            assert_eq!(focused(cx), start, "a tap on the filter field moved focus");
        }
        send(&input, &view, cx);
        seen.push((sent(&view, &receipts, cx), terminal_focused(&view, cx)));
    }
    assert_eq!(
        view.read_with(cx, |this, _| this.sidebar_filter().to_owned()),
        "",
        "a key reached the filter query"
    );
    assert_eq!(
        fake.presents(),
        usize::from(tap),
        "fixture: the taps opened one panel"
    );
    seen
}

/// A tap on the magnifier opens the panel and keeps focus where it was, in
/// every state the earlier field changed with: no hardware keyboard, a
/// hardware keyboard, and Herdr 2. No GPUI input field is drawn.
#[gpui::test]
fn the_magnifier_opens_search_and_keeps_focus(cx: &mut TestAppContext) {
    for (hardware, core) in [(false, false), (true, false), (true, true)] {
        let (view, receipts, cx) = terminal(cx, hardware);
        let fake = crate::shell::search::tests::install_fake(&view, cx);
        view.update(cx, |this, cx| {
            this.terminal.core_mode = core;
            cx.notify();
        });
        cx.run_until_parked();
        let focus = terminal_focused(&view, cx);
        tap_field(cx);
        assert_eq!(
            fake.presents(),
            1,
            "hardware={hardware} core={core}: the panel was not presented"
        );
        assert_eq!(
            terminal_focused(&view, cx),
            focus,
            "hardware={hardware} core={core}: the tap moved focus"
        );
        assert!(
            sent(&view, &receipts, cx).is_empty(),
            "hardware={hardware} core={core}: the tap sent input"
        );
        assert!(
            cx.debug_bounds("sidebar-filter").is_none(),
            "hardware={hardware} core={core}: an input field was drawn"
        );
    }
}

/// With a hardware keyboard known (the state in which the earlier field
/// took the keys), tapping the field before every input changes neither
/// what the terminal receives nor which element has focus.
#[gpui::test]
fn a_tap_on_the_field_changes_no_terminal_input_with_a_hardware_keyboard(cx: &mut TestAppContext) {
    let untouched = run(cx, true, false);
    let tapped = run(cx, true, true);
    assert!(
        untouched.iter().any(|(events, _)| !events.is_empty()),
        "fixture: the terminal received none of the inputs"
    );
    assert_eq!(tapped, untouched);
}

/// The same without a hardware keyboard (the software keyboard is the input).
#[gpui::test]
fn a_tap_on_the_field_changes_no_terminal_input_without_a_hardware_keyboard(
    cx: &mut TestAppContext,
) {
    let untouched = run(cx, false, false);
    let tapped = run(cx, false, true);
    assert_eq!(tapped, untouched);
}
