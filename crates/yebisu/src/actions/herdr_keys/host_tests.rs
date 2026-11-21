use super::super::tests::parse;
use super::*;
use gpui::{AppContext, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, Window};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::time::{Duration, Instant};

fn attach(target: &str, terminal: &TerminalView) -> Connection {
    Connection {
        inbox: Arc::downgrade(&terminal.link.inbox),
        target: target.to_owned(),
    }
}

fn install_profile(this: &mut Smoke) {
    let mut config = HostKeys {
        connection: Some(attach(&this.terminal.reconnect.target, &this.terminal)),
        ..HostKeys::default()
    };
    config.result = Some(Ok(parse("[keys]\nprefix = 'ctrl+a'\nnew_tab = 'prefix+g'")));
    this.actions.host_keys = config;
}

#[test]
fn herdr_prefix_reader_opens_only_a_bounded_regular_file() -> std::io::Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("herdr-key-reader-{}", std::process::id()));
    let path = root.join(".config/herdr/config.toml");
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing config directory"))?;
    std::fs::create_dir_all(dir)?;
    let command = || {
        std::process::Command::new("sh")
            .args(["-c", READ_CONFIG])
            .env("HOME", &root)
            .output()
    };
    let missing = command()?;
    assert_eq!(missing.status.code(), Some(44));
    assert!(missing.stdout.is_empty());

    std::fs::create_dir(&path)?;
    assert_eq!(command()?.status.code(), Some(45));
    std::fs::remove_dir(&path)?;

    let content = b"[keys]\nprefix = 'ctrl+a'\n";
    std::fs::write(&path, content)?;
    let output = command()?;
    assert!(output.status.success());
    assert_eq!(output.stdout, content);
    std::fs::write(&path, vec![b' '; MAX_BYTES + 25])?;
    assert_eq!(command()?.stdout.len(), MAX_BYTES + 1);
    std::fs::remove_dir_all(root)
}

#[test]
fn herdr_prefix_row_identifies_the_host_and_never_leaks_the_previous_config() {
    let mut terminal = TerminalView::default();
    terminal.reconnect.target = "operator@pro-dev.example.test".into();
    terminal.live.connected = true;
    let mut keys = HostKeys {
        connection: Some(attach(&terminal.reconnect.target, &terminal)),
        result: Some(Ok(parse("[keys]\nprefix = 'ctrl+a'"))),
        ..Default::default()
    };
    let (value, detail) = keys.prefix_row(&terminal);
    assert_eq!(value, "ctrl+a");
    assert!(detail.contains("pro-dev"));
    assert!(detail.contains("[keys].prefix"));
    for error in [
        Fallback::MissingFile,
        Fallback::InvalidToml,
        Fallback::Timeout,
        Fallback::NotRegular,
        Fallback::PermissionDenied,
    ] {
        keys.result = Some(Err(error));
        let (value, detail) = keys.prefix_row(&terminal);
        assert_eq!(value, "ctrl+b");
        assert!(detail.contains(error.reason()), "{detail}");
    }
    keys.result = Some(Ok(parse("[keys]\nprefix = 'ctrl+a'")));
    terminal.link.reset();
    terminal.reconnect.target = "another.example.test".into();
    let (value, detail) = keys.prefix_row(&terminal);
    assert_eq!(value, "ctrl+b");
    assert!(!detail.contains("pro-dev"), "{detail}");
    assert!(detail.contains("another"), "{detail}");
}

fn press(this: &mut Smoke, name: &'static str, window: &mut Window, cx: &mut Context<Smoke>) {
    let keystroke = Keystroke::parse(name).unwrap_or_else(|error| panic!("key: {error}"));
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
}

fn wait_for_terminal(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext) {
    let until = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.connected && this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < until, "terminal did not connect");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn herdr_prefix_adapter_sends_only_replay_and_ipad_passthrough_before_the_tab_rpc(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait_for_terminal(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            install_profile(this);
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert!(this.terminal.input.ready);
            press(this, "ctrl-a", window, cx); // iPad default: the prefix passes through.
            this.actions.herdr_prefix = true;
            press(this, "ctrl-a", window, cx);
            press(this, "ctrl-u", window, cx); // An unknown chord never reaches the peer.
            press(this, "ctrl-a", window, cx);
            press(this, "ctrl-a", window, cx); // Exactly one Ctrl+A from the pair.
            press(this, "ctrl-a", window, cx);
            press(this, "g", window, cx); // A server override, distinct from the default c.
        })
    });

    let mut keys = Vec::new();
    loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|error| panic!("no tab.create RPC: {error}"));
        match message {
            ClientMessage::ClientShellResize { .. } | ClientMessage::ClientShellFocus { .. } => {}
            ClientMessage::ClientShellPaneInput { events, .. } => {
                for event in events {
                    let ClientPaneInputEvent::Key {
                        code,
                        kind,
                        modifiers,
                        ..
                    } = event
                    else {
                        panic!("unexpected input: {event:?}");
                    };
                    assert_eq!(modifiers, 2);
                    if kind == ClientKeyKind::Press {
                        keys.push(code);
                    }
                }
            }
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let rpc: serde_json::Value = serde_json::from_str(&request)
                    .unwrap_or_else(|error| panic!("request: {error}"));
                assert_eq!(rpc["method"], "tab.create");
                break;
            }
            message => panic!("unexpected message: {message:?}"),
        }
    }
    assert_eq!(keys, [ClientKeyCode::Char('a'), ClientKeyCode::Char('a')]);
}

#[gpui::test]
fn herdr_prefix_adapter_creates_a_tab_in_a_herdr2_session(cx: &mut TestAppContext) {
    use crate::{core::Session, shell::HerdrKind};
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
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
    wait_for_terminal(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            install_profile(this);
            this.actions.herdr_prefix = true;
            assert_eq!(this.core_tabs().len(), 1);
            press(this, "ctrl-a", window, cx);
            press(this, "g", window, cx);
        })
    });
    let until = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.core_tabs().len() == 2
    }) {
        assert!(
            Instant::now() < until,
            "Herdr 2 did not publish the new tab"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn herdr_prefix_timeout_discards_late_completion(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        this.terminal.live.connected = true;
        let (send, receive) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let keys = &mut this.actions.host_keys;
        keys.connection = Some(attach(&this.terminal.reconnect.target, &this.terminal));
        keys.pending = Some(Pending {
            deadline: Instant::now(),
            stop: stop.clone(),
            receive,
        });
        keys.poll(&this.terminal, true, cx);
        assert!(stop.load(Ordering::Acquire));
        assert!(
            keys.pending.is_some(),
            "a cancelled worker retains its slot"
        );
        send.try_send(Ok(parse("[keys]\nprefix = 'ctrl+a'")))
            .unwrap_or_else(|error| panic!("late completion: {error}"));
        keys.poll(&this.terminal, true, cx);
        assert!(keys.pending.is_none());
        let (value, detail) = keys.prefix_row(&this.terminal);
        assert_eq!(value, "ctrl+b");
        assert!(detail.contains("timed out"), "{detail}");
    });
}

#[gpui::test]
fn herdr_prefix_help_and_ipad_shortcut_open_the_same_popover(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let client = crate::mock_peer::connect().unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait_for_terminal(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            install_profile(this);
            press(this, "cmd-/", window, cx);
            assert!(this.shell.shortcuts.open, "iPad help");
            press(this, "escape", window, cx);
            this.actions.herdr_prefix = true;
            press(this, "ctrl-a", window, cx);
            press(this, "?", window, cx);
            assert!(this.shell.shortcuts.open, "Herdr help");
        })
    });
}
