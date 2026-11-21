use super::super::tests::parse;
use super::*;
use gpui::{AppContext, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, Window};
use sheprd_client::protocol::{
    ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent, ClientShellPopupSurface,
};
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
            .env_remove("XDG_CONFIG_HOME")
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
fn herdr_config_reader_uses_only_absolute_xdg_config_home() -> std::io::Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("herdr-config-home-{}", std::process::id()));
    let alternate = root.join("custom config $(literal)");
    let home_config = b"[keys]\nprefix = 'ctrl+a'\n";
    let xdg_config = b"[keys]\nprefix = 'ctrl+c'\n";
    for (dir, content) in [
        (root.join(".config/herdr"), home_config),
        (alternate.join("herdr"), xdg_config),
    ] {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("config.toml"), content)?;
    }
    let command = |xdg: Option<&std::path::Path>| {
        let mut child = std::process::Command::new("sh");
        child.args(["-c", READ_CONFIG]).env("HOME", &root);
        if let Some(path) = xdg {
            child.env("XDG_CONFIG_HOME", path);
        } else {
            child.env_remove("XDG_CONFIG_HOME");
        }
        child.output()
    };
    for (xdg, expected) in [
        (None, home_config),
        (Some(std::path::Path::new("")), home_config),
        (Some(std::path::Path::new("relative-config")), home_config),
        (Some(alternate.as_path()), xdg_config),
    ] {
        let output = command(xdg)?;
        assert!(output.status.success(), "{xdg:?}: {:?}", output.status);
        assert_eq!(output.stdout, expected, "{xdg:?}");
    }
    let missing = command(Some(&root.join("missing")))?;
    assert_eq!(missing.status.code(), Some(44));
    assert!(missing.stdout.is_empty());
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

fn shortcut_window(cx: &mut TestAppContext) -> (gpui::Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        this.terminal.open = true;
        this.actions.herdr_prefix = true;
        cx.notify();
    });
    (view, cx)
}

fn connect_shortcut_peer(
    view: &gpui::Entity<Smoke>,
    cx: &mut VisualTestContext,
) -> mpsc::Receiver<ClientMessage> {
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait_for_terminal(view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            install_profile(this);
            cx.notify();
        });
    });
    receipts
}

#[track_caller]
fn command_comma_opens_settings(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("cmd-,");
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(this.settings_modal, "⌘, did not open Settings");
            assert!(this.settings_focus.is_focused(window));
        })
    });
}

#[gpui::test]
fn herdr_settings_shortcut_connected(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let _receipts = connect_shortcut_peer(&view, cx);
    // The exception takes precedence even over an armed Herdr prefix.
    cx.simulate_keystrokes("ctrl-a");
    command_comma_opens_settings(&view, cx);
}

#[gpui::test]
fn herdr_settings_shortcut_disconnected_without_focus(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    cx.update(|window, cx| window.blur(cx));
    cx.simulate_keystrokes("cmd-shift-, ctrl-cmd-,");
    view.update(cx, |this, _| {
        assert!(!this.terminal.live.connected);
        assert!(
            !this.settings_modal,
            "extra modifiers must not open Settings"
        );
    });
    command_comma_opens_settings(&view, cx);
}

#[gpui::test]
fn herdr_settings_shortcut_with_reconnect_pending(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    view.update(cx, |this, cx| {
        this.terminal.reconnect_pending = true;
        cx.notify();
    });
    command_comma_opens_settings(&view, cx);
    view.update(cx, |this, _| assert!(this.terminal.reconnect_pending));
}

#[gpui::test]
fn herdr_settings_shortcut_over_palette(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let _receipts = connect_shortcut_peer(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.actions.palette = true;
            window.blur(cx);
            cx.notify();
        });
    });
    command_comma_opens_settings(&view, cx);
    view.update(cx, |this, _| assert!(!this.actions.palette));
}

#[gpui::test]
fn herdr_settings_shortcut_over_host_popup(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let _receipts = connect_shortcut_peer(&view, cx);
    view.update(cx, |this, cx| {
        let surface = this
            .terminal
            .live
            .surface
            .as_mut()
            .map(Arc::make_mut)
            .unwrap_or_else(|| panic!("missing peer surface"));
        surface.popup = Some(Box::new(ClientShellPopupSurface {
            terminal_id: "popup".into(),
            title: "test popup".into(),
            width: None,
            height: None,
            frame: surface.frame.clone(),
            mouse_reporting: false,
            sgr_pixel_mouse: false,
            pixel_width: 0,
            pixel_height: 0,
        }));
        assert!(!crate::actions::catalog_context_available(&this.terminal));
        cx.notify();
    });
    cx.update(|window, cx| window.blur(cx));
    command_comma_opens_settings(&view, cx);
}

#[gpui::test]
fn herdr_settings_shortcut_over_shortcuts_popover(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let _receipts = connect_shortcut_peer(&view, cx);
    cx.simulate_keystrokes("ctrl-a ?");
    view.update(cx, |this, _| assert!(this.shell.shortcuts.open));
    command_comma_opens_settings(&view, cx);
    view.update(cx, |this, _| assert!(!this.shell.shortcuts.open));
}

#[gpui::test]
fn herdr_settings_is_the_only_native_app_shortcut(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let _receipts = connect_shortcut_peer(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(crate::actions::catalog_context_available(&this.terminal));
            // Include every supported catalog shortcut and the two special cases.
            let bindings = crate::actions::catalog(&this.terminal)
                .into_iter()
                .flat_map(|action| action.shortcuts.iter().copied())
                .chain(["cmd-k", "cmd-/"]);
            for binding in bindings {
                let keystroke =
                    Keystroke::parse(binding).unwrap_or_else(|error| panic!("{binding}: {error}"));
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                assert!(!this.actions.consumed.contains(&keystroke.key), "{binding}");
                assert!(!this.actions.palette, "{binding}");
                assert!(this.find.is_none(), "{binding}");
                assert!(!this.shell.shortcuts.open, "{binding}");
                this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
            }
        });
    });
    command_comma_opens_settings(&view, cx);
}

#[gpui::test]
fn herdr_settings_shortcut_preserves_copy_and_paste(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let receipts = connect_shortcut_peer(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.sync_handoff();
            assert!(this.terminal.input.ready);
            let frame = this
                .terminal
                .live
                .surface
                .as_ref()
                .map(|surface| Arc::new(surface.frame.clone()))
                .unwrap_or_else(|| panic!("missing peer surface"));
            this.handoff.selection = Some(crate::handoff::Selection {
                frame,
                anchor: (1, 2),
                end: (6, 2),
                dragging: false,
            });
            press(this, "cmd-c", window, cx);
            assert_eq!(this.handoff.clipboard_writes, 1);
            press(this, "cmd-v", window, cx);
            assert_eq!(this.handoff.clipboard_reads, 1);
        });
    });
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("日本語")
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "peer did not receive paste");
        if let Ok(ClientMessage::ClientShellPaneInput { pane_id, events }) =
            receipts.recv_timeout(Duration::from_millis(5))
        {
            assert_eq!(pane_id, "w1:p1");
            assert_eq!(events, [ClientPaneInputEvent::Paste("日本語".into())]);
            break;
        }
    }
    command_comma_opens_settings(&view, cx);
}

#[gpui::test]
fn herdr_settings_shortcut_uses_the_ipad_terminal_route_once(cx: &mut TestAppContext) {
    let (view, cx) = shortcut_window(cx);
    let _receipts = connect_shortcut_peer(&view, cx);
    view.update(cx, |this, _| this.actions.herdr_prefix = false);
    cx.simulate_keystrokes("cmd-/");
    view.update(cx, |this, _| {
        assert!(this.shell.shortcuts.open, "iPad help shortcut")
    });
    cx.simulate_keystrokes("escape");
    view.update(cx, |this, _| assert!(!this.shell.shortcuts.open));
    command_comma_opens_settings(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| this.end_settings_modal(window, cx));
    });
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(!this.settings_modal, "one close dismissed Settings");
        assert!(!this.actions.herdr_prefix);
    });
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
