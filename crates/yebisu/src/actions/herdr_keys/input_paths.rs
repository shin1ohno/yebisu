//! Herdr prefix style on each way a key reaches the terminal (L57): a
//! hardware press (KeyDown), text UIKit types without a KeyDown (the
//! software keyboard, or an IME that took a hardware key first), and the
//! key band's Control latch.
use crate::{app::Smoke, core::Session, mock_peer, shell::HerdrKind};
use gpui::{
    Bounds, Edges, Entity, EntityInputHandler, InputHandler, KeyDownEvent, KeyUpEvent, Keystroke,
    Modifiers, TestAppContext, VisualTestContext, WindowInsets, point, px, size,
};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const CTRL_A: &str = "[keys]\nprefix = 'ctrl+a'\n";

/// What the Herdr 1 peer received, in order, up to the first RPC.
#[derive(Debug, Default, PartialEq)]
struct Received {
    keys: Vec<(ClientKeyCode, u8)>,
    text: Vec<String>,
    rpc: Option<String>,
}

fn receive(receipts: &Receiver<ClientMessage>, quiet: Duration) -> Received {
    let mut received = Received::default();
    while let Ok(message) = receipts.recv_timeout(quiet) {
        match message {
            ClientMessage::ClientShellPaneInput { events, .. } => {
                for event in events {
                    match event {
                        ClientPaneInputEvent::Key {
                            code,
                            kind: ClientKeyKind::Press,
                            modifiers,
                            ..
                        } => received.keys.push((code, modifiers)),
                        ClientPaneInputEvent::TextCommit(text) => received.text.push(text),
                        _ => {}
                    }
                }
            }
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let rpc: serde_json::Value = serde_json::from_str(&request)
                    .unwrap_or_else(|error| panic!("request: {error}"));
                received.rpc = rpc["method"].as_str().map(str::to_owned);
                break;
            }
            _ => {}
        }
    }
    received
}

fn new_tab() -> Received {
    Received {
        rpc: Some("tab.create".into()),
        ..Received::default()
    }
}

fn window(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx)
}

fn wait_until(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    what: &str,
    ready: impl Fn(&mut Smoke) -> bool,
) {
    let until = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.refresh_terminal_input();
        ready(this)
    }) {
        assert!(Instant::now() < until, "{what}");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
}

/// A connected Herdr 1 terminal with focus, the host's `[keys].prefix`
/// ctrl+a loaded, and a docked software keyboard, so the key band shows.
fn herdr1(
    cx: &mut TestAppContext,
    herdr_prefix: bool,
) -> (
    Entity<Smoke>,
    &mut VisualTestContext,
    Receiver<ClientMessage>,
) {
    let (view, cx) = window(cx);
    let (client, receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    wait_until(&view, cx, "Herdr 1 terminal did not connect", |this| {
        this.terminal.live.connected && this.terminal.live.surface.is_some()
    });
    software_keyboard(&view, cx);
    focus(&view, cx, herdr_prefix);
    (view, cx, receipts)
}

fn software_keyboard(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(WindowInsets {
            ime: Edges {
                bottom: px(498.),
                ..Edges::default()
            },
            ..WindowInsets::default()
        });
        this.shell.fit.hardware_keyboard_attached = false;
        cx.notify();
    });
    cx.run_until_parked();
    wait_until(view, cx, "the terminal input is not ready", |this| {
        this.terminal.input.ready
    });
}

fn focus(view: &Entity<Smoke>, cx: &mut VisualTestContext, herdr_prefix: bool) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.actions.herdr_prefix = herdr_prefix;
            let terminal = &this.terminal;
            this.actions.host_keys.set_test_config(terminal, CTRL_A);
            cx.notify();
        });
    });
    cx.run_until_parked();
}

/// A hardware press: UIKit's `pressesBegan:` with the Control modifier
/// reaches GPUI as a KeyDown (herdr-uikit keys.rs `presses_began`).
fn press(view: &Entity<Smoke>, cx: &mut VisualTestContext, name: &'static str) {
    let keystroke = Keystroke::parse(name).unwrap_or_else(|error| panic!("{name}: {error}"));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_key_down(
                &KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
        });
    });
    cx.run_until_parked();
}

/// Text UIKit types with no KeyDown: `insertText:` from the software
/// keyboard, or from the IME for a hardware key it took first.
fn typed(view: &Entity<Smoke>, cx: &mut VisualTestContext, text: &str) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_text_in_range(None, text, window, cx);
        });
    });
    cx.run_until_parked();
}

fn tap(cx: &mut VisualTestContext, id: &'static str) {
    let bounds = cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("{id} is absent"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

const QUIET: Duration = Duration::from_millis(500);

#[gpui::test]
fn key_band_control_latch_and_software_letters_create_a_tab(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    typed(&view, cx, "c");
    assert_eq!(receive(&receipts, QUIET), new_tab());
}

#[gpui::test]
fn hardware_prefix_then_a_letter_typed_through_uikit_creates_a_tab(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    press(&view, cx, "ctrl-a");
    typed(&view, cx, "c");
    assert_eq!(receive(&receipts, QUIET), new_tab());
}

#[gpui::test]
fn committed_control_character_arms_the_prefix(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    typed(&view, cx, "\u{1}");
    typed(&view, cx, "c");
    assert_eq!(receive(&receipts, QUIET), new_tab());
}

#[gpui::test]
fn hardware_prefix_then_hardware_letter_creates_a_tab(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    press(&view, cx, "ctrl-a");
    press(&view, cx, "c");
    assert_eq!(receive(&receipts, QUIET), new_tab());
}

#[gpui::test]
fn key_band_prefix_twice_sends_one_control_a(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    // The replay ended the prefix: c is typing again.
    typed(&view, cx, "c");
    assert_eq!(
        receive(&receipts, QUIET),
        Received {
            keys: vec![(ClientKeyCode::Char('a'), 2)],
            text: vec!["c".into()],
            rpc: None,
        }
    );
}

#[gpui::test]
fn typed_prefix_twice_sends_one_control_a(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    press(&view, cx, "ctrl-a");
    typed(&view, cx, "\u{1}");
    assert_eq!(
        receive(&receipts, QUIET),
        Received {
            text: vec!["\u{1}".into()],
            ..Received::default()
        }
    );
}

#[gpui::test]
fn ipad_style_sends_control_a_from_every_path(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, false);
    press(&view, cx, "ctrl-a");
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    typed(&view, cx, "c");
    assert_eq!(
        receive(&receipts, QUIET),
        Received {
            keys: vec![(ClientKeyCode::Char('a'), 2), (ClientKeyCode::Char('a'), 2)],
            text: vec!["c".into()],
            rpc: None,
        }
    );
}

#[gpui::test]
fn key_band_keys_complete_a_prefix(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    view.update(cx, |this, _| {
        let terminal = &this.terminal;
        this.actions.host_keys.set_test_config(
            terminal,
            "[keys]\nprefix = 'ctrl+a'\nnew_tab = 'prefix+tab'",
        );
    });
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    tap(cx, "key-band-tab");
    assert_eq!(receive(&receipts, QUIET), new_tab());
}

#[gpui::test]
fn the_default_prefix_applies_while_the_host_keys_load(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    // A reconnect drops the loaded keys; no read result has arrived yet.
    view.update(cx, |this, _| this.actions.host_keys = Default::default());
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    typed(&view, cx, "c");
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "b");
    typed(&view, cx, "c");
    assert_eq!(
        receive(&receipts, QUIET),
        Received {
            keys: vec![(ClientKeyCode::Char('a'), 2)],
            text: vec!["c".into()],
            rpc: Some("tab.create".into()),
        }
    );
}

/// The handler the terminal registers with the platform (terminal_view.rs).
fn terminal_handler(view: &Entity<Smoke>) -> impl InputHandler {
    super::TerminalInputHandler::new(
        Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))),
        view.clone(),
    )
}

#[gpui::test]
fn an_armed_prefix_keeps_printable_keys_out_of_the_ime(cx: &mut TestAppContext) {
    let (view, cx, _receipts) = herdr1(cx, true);
    let prefers = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| terminal_handler(&view).prefers_ime_for_printable_keys(window, cx))
    };
    assert!(prefers(cx), "typing composes as before");
    press(&view, cx, "ctrl-a");
    assert!(!prefers(cx), "c must reach the prefix as a key press");
    // An unbound second key ends the chord without opening a tab.
    press(&view, cx, "u");
    assert!(prefers(cx), "the chord ended");
}

fn wait_for_tabs(view: &Entity<Smoke>, cx: &mut VisualTestContext, count: usize) {
    wait_until(view, cx, "Herdr 2 did not publish the new tab", |this| {
        this.core_tabs().len() == count
    });
}

#[gpui::test]
fn herdr2_creates_a_tab_from_the_key_band_and_typed_letters(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let (backend, _receipts) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host_kind("private-core", HerdrKind::Herdr2);
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core = Some(
            Session::start(
                "private-core".into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                None,
                crate::shell::single_pane::Mailbox::detached(),
            )
            .unwrap_or_else(|error| panic!("session: {error}")),
        );
    });
    wait_until(&view, cx, "Herdr 2 terminal did not connect", |this| {
        this.terminal.live.connected && this.terminal.live.surface.is_some()
    });
    software_keyboard(&view, cx);
    focus(&view, cx, true);
    wait_for_tabs(&view, cx, 1);
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    typed(&view, cx, "c");
    wait_for_tabs(&view, cx, 2);
    press(&view, cx, "ctrl-a");
    typed(&view, cx, "c");
    wait_for_tabs(&view, cx, 3);
}
