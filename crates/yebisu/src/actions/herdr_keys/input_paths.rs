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

fn terminal_priority_with_command_as_control(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, _| {
        this.actions.terminal_priority = true;
        this.terminal.input.options.command_control = true;
    });
}

/// L57 device result: c was taken and no tab appeared. Terminal priority with
/// Command as Control makes the OS menu rows unavailable, and the chord used
/// that menu gate.
#[gpui::test]
fn terminal_priority_with_command_as_control_keeps_herdr_chords(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    terminal_priority_with_command_as_control(&view, cx);
    press(&view, cx, "ctrl-a");
    press(&view, cx, "c");
    assert_eq!(receive(&receipts, QUIET), new_tab(), "hardware keyboard");
    wait_until(&view, cx, "the first tab.create did not finish", |this| {
        this.terminal.navigating.is_none()
    });
    tap(cx, "key-band-ctrl");
    typed(&view, cx, "a");
    typed(&view, cx, "c");
    assert_eq!(
        receive(&receipts, QUIET),
        new_tab(),
        "key band and software keyboard"
    );
    // What Shortcut priority = Terminal is for: a ⌘ chord still reaches
    // the terminal, as Control with Command key = Ctrl.
    wait_until(&view, cx, "the second tab.create did not finish", |this| {
        this.terminal.navigating.is_none() && this.terminal.input.ready
    });
    press(&view, cx, "cmd-t");
    assert_eq!(
        receive(&receipts, QUIET),
        Received {
            keys: vec![(ClientKeyCode::Char('t'), 2)],
            ..Received::default()
        },
        "cmd-t goes to the terminal"
    );
}

/// L57: the chord's error was written to the live status, which the next
/// live update replaced, so a failed chord looked like nothing happened.
#[gpui::test]
fn a_failed_herdr_chord_keeps_its_reason_visible(cx: &mut TestAppContext) {
    let (view, cx, receipts) = herdr1(cx, true);
    cx.deactivate_window();
    press(&view, cx, "ctrl-a");
    press(&view, cx, "c");
    assert_eq!(receive(&receipts, QUIET), Received::default());
    let live_update = |view: &Entity<Smoke>, cx: &mut VisualTestContext| {
        view.update(cx, |this, cx| {
            if let Ok(mut live) = this.terminal.link.inbox.lock() {
                live.dirty = true;
            }
            this.poll_terminal(cx);
            this.terminal_status()
        })
    };
    let status = live_update(&view, cx);
    assert!(status.contains("inactive terminal focus"), "{status}");
    cx.update(|window, _| window.activate_window());
    press(&view, cx, "x");
    assert_eq!(
        live_update(&view, cx),
        "Connected",
        "the next key clears it"
    );
}

/// A real Herdr 1 server in a private directory: cleared environment, its own
/// HOME, XDG and socket paths. Drop kills and reaps only this child.
struct Herdr1 {
    dir: std::path::PathBuf,
    child: Option<std::process::Child>,
}

impl Herdr1 {
    fn start(binary: &std::ffi::OsStr, config: &str) -> Self {
        let parent =
            std::env::var_os("YEBISU_HERDR1_TMPDIR").map_or_else(std::env::temp_dir, Into::into);
        let dir = parent.join(format!("y{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap_or_else(|error| panic!("sandbox: {error}"));
        std::fs::write(dir.join("config.toml"), config).unwrap_or_else(|error| panic!("{error}"));
        let log =
            std::fs::File::create(dir.join("daemon.log")).unwrap_or_else(|error| panic!("{error}"));
        let err = log.try_clone().unwrap_or_else(|error| panic!("{error}"));
        let child = std::process::Command::new(binary)
            .arg("server")
            .env_clear()
            .env("HOME", &dir)
            .env("XDG_CONFIG_HOME", dir.join("config"))
            .env("XDG_STATE_HOME", dir.join("state"))
            .env("XDG_DATA_HOME", dir.join("data"))
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .env("XDG_RUNTIME_DIR", &dir)
            .env("TMPDIR", &dir)
            .env("HERDR_CONFIG_PATH", dir.join("config.toml"))
            .env("HERDR_SOCKET_PATH", dir.join("a.sock"))
            .env("HERDR_CLIENT_SOCKET_PATH", dir.join("a-client.sock"))
            .env("SHELL", "/bin/sh")
            .env("TERM", "xterm-256color")
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&dir)
            .stdin(std::process::Stdio::null())
            .stdout(log)
            .stderr(err)
            .spawn()
            .unwrap_or_else(|error| panic!("herdr server: {error}"));
        let server = Self {
            dir,
            child: Some(child),
        };
        let until = Instant::now() + Duration::from_secs(20);
        while !server.dir.join("a-client.sock").exists() {
            assert!(
                Instant::now() < until,
                "Herdr 1 did not listen: {}",
                std::fs::read_to_string(server.dir.join("daemon.log")).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        server
    }
}

impl Drop for Herdr1 {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn shown_tabs(this: &Smoke) -> Vec<String> {
    crate::board::snapshot(&this.terminal.live)
        .map(|s| s.tabs.iter().map(|tab| tab.tab_id.clone()).collect())
        .unwrap_or_default()
}

/// L57 device result: on a real Herdr 1 the prefix took c but no tab appeared.
/// Opt-in: YEBISU_HERDR1_BINARY names the Herdr 1 executable to sandbox, and
/// YEBISU_HERDR1_CONFIG optionally a config.toml for the server and host keys.
#[gpui::test]
#[ignore = "needs YEBISU_HERDR1_BINARY; starts a private Herdr 1 server"]
fn real_herdr1_creates_a_tab_from_prefix_c(cx: &mut TestAppContext) {
    real_herdr1_prefix_c(cx, false);
}

/// The same with Terminal priority and Command as Control, the settings under
/// which the device result reproduced.
#[gpui::test]
#[ignore = "needs YEBISU_HERDR1_BINARY; starts a private Herdr 1 server"]
fn real_herdr1_creates_a_tab_from_prefix_c_with_command_as_control(cx: &mut TestAppContext) {
    real_herdr1_prefix_c(cx, true);
}

fn real_herdr1_prefix_c(cx: &mut TestAppContext, command_as_control: bool) {
    let binary =
        std::env::var_os("YEBISU_HERDR1_BINARY").unwrap_or_else(|| panic!("YEBISU_HERDR1_BINARY"));
    let config = std::env::var("YEBISU_HERDR1_CONFIG").map_or_else(
        |_| "onboarding = false\n[keys]\nprefix = \"ctrl+a\"\n[terminal]\ndefault_shell = \"/bin/sh\"\nshell_mode = \"non_login\"\n".to_owned(),
        |path| std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{error}")),
    );
    let server = Herdr1::start(&binary, &config);
    let client = sheprd_client::connect(
        sheprd_client::ConnectTarget::Socket(server.dir.join("a-client.sock")),
        sheprd_client::ConnectOptions::default(),
    )
    .unwrap_or_else(|error| panic!("connect: {error}"));
    let (view, cx) = window(cx);
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    wait_until(&view, cx, "Herdr 1 terminal did not connect", |this| {
        this.terminal.live.connected && this.terminal.live.surface.is_some()
    });
    focus(&view, cx, true);
    // The host keys come from the same config the server reads.
    view.update(cx, |this, _| {
        let terminal = &this.terminal;
        this.actions.host_keys.set_test_config(terminal, &config);
    });
    if command_as_control {
        terminal_priority_with_command_as_control(&view, cx);
    }
    let before = view.update(cx, |this, _| shown_tabs(this));
    eprintln!("L57 before: tabs={before:?}");
    press(&view, cx, "ctrl-a");
    press(&view, cx, "c");
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let (tabs, status, navigating) = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            (
                shown_tabs(this),
                this.terminal.live.status.clone(),
                this.terminal.navigating.is_some(),
            )
        });
        if tabs.len() > before.len() {
            eprintln!("L57 after: tabs={tabs:?}");
            break;
        }
        assert!(
            Instant::now() < until,
            "no tab: tabs={tabs:?} status={status:?} navigating={navigating}"
        );
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
}

/// The status line after a live update, which replaces the live status.
fn status_after_update(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.update(cx, |this, cx| {
        if let Ok(mut live) = this.terminal.link.inbox.lock() {
            live.dirty = true;
        }
        this.poll_terminal(cx);
        this.terminal_status()
    })
}

/// ctrl+a, c while the window is inactive fails with a reason.
fn fail_chord(view: &Entity<Smoke>, cx: &mut VisualTestContext, before: &str) {
    cx.deactivate_window();
    press(view, cx, "ctrl-a");
    press(view, cx, "c");
    cx.update(|window, _| window.activate_window());
    let status = status_after_update(view, cx);
    assert!(
        status.contains("inactive terminal focus"),
        "before {before}: {status}"
    );
}

/// L57 review F1: the reason must end with any input that types into the
/// terminal, and must never show for another host or connection.
#[gpui::test]
fn a_chord_failure_reason_ends_with_typing_actions_and_reconnects(cx: &mut TestAppContext) {
    let (view, cx, _receipts) = herdr1(cx, true);
    // Every case runs, so a failure lists each path that keeps the reason.
    let mut kept = Vec::new();
    let mut gone = |view: &Entity<Smoke>, cx: &mut VisualTestContext, what: &str| {
        let status = status_after_update(view, cx);
        if status.contains("inactive terminal focus") {
            kept.push(format!("{what}: {status}"));
        }
    };
    fail_chord(&view, cx, "software keyboard text");
    typed(&view, cx, "l");
    gone(&view, cx, "software keyboard text");

    fail_chord(&view, cx, "IME composition");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_and_mark_text_in_range(None, "k", None, window, cx);
            this.unmark_text(window, cx);
        });
    });
    gone(&view, cx, "IME composition");

    fail_chord(&view, cx, "key band key");
    tap(cx, "key-band-esc");
    gone(&view, cx, "key band key");

    fail_chord(&view, cx, "tab row, palette or menu action");
    view.update(cx, |this, _| {
        let _ = this.run_action(sheprd_commands::Command::ToggleSidebar);
    });
    gone(&view, cx, "tab row, palette or menu action");

    fail_chord(&view, cx, "a setting change");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.begin_settings_modal(window, cx);
            this.end_settings_modal(window, cx);
        });
    });
    gone(&view, cx, "a setting change");

    // Only the host name changes here; a real switch also dials a new link.
    fail_chord(&view, cx, "another host");
    let host = view.update(cx, |this, _| {
        std::mem::replace(&mut this.terminal.reconnect.target, "host-b".into())
    });
    gone(&view, cx, "another host");
    view.update(cx, |this, _| this.terminal.reconnect.target = host);

    // Last: a new connection pauses text input until the next gesture.
    fail_chord(&view, cx, "a new connection");
    view.update(cx, |this, _| {
        this.terminal.link.reset();
    });
    gone(&view, cx, "a new connection");
    assert!(
        kept.is_empty(),
        "reason still shown after:\n{}",
        kept.join("\n")
    );
}
