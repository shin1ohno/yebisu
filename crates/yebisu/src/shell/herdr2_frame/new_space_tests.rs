use super::*;

#[gpui::test]
fn new_space_asks_for_the_host_and_the_herdr(cx: &mut gpui::TestAppContext) {
    let setup = crate::hosts::test_support::named_setup();
    let hosts = host_choices(
        &setup,
        [
            ("dev@mini.local".into(), HerdrKind::Herdr1),
            ("dev@studio.local".into(), HerdrKind::Herdr1),
            ("dev@studio.local".into(), HerdrKind::Herdr2),
        ],
    );
    let mut chooser = Chooser::new(hosts);
    assert_eq!(
        chooser.selection(),
        None,
        "the host must be chosen on every invocation"
    );
    chooser.select_host(0);
    assert_eq!(chooser.hosts[0].name.display(), "Studio Mac");
    assert_eq!(
        chooser.selection(),
        Some(("dev@studio.local", HerdrKind::Herdr2))
    );
    chooser.select_kind(HerdrKind::Herdr1);
    assert_eq!(
        chooser.selection(),
        Some(("dev@studio.local", HerdrKind::Herdr1))
    );
    assert_eq!(chooser.hosts[1].name.display(), "mini");
    chooser.select_host(1);
    chooser.select_kind(HerdrKind::Herdr2);
    assert_eq!(
        chooser.selection(),
        Some(("dev@mini.local", HerdrKind::Herdr1))
    );
    chooser.select_host(0);
    assert_eq!(
        chooser.selection(),
        Some(("dev@studio.local", HerdrKind::Herdr2))
    );

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        this.setup = crate::hosts::test_support::named_setup();
        this.terminal.reconnect.target = "dev@studio.local".into();
        this.terminal.open = true;
        this.terminal.live.connected = true;
        this.shell
            .herdr2
            .confirm_session("dev@studio.local", "default");
        this.open_new_space(cx);
    });
    cx.run_until_parked();
    let row = cx
        .debug_bounds("new-space-host-0")
        .unwrap_or_else(|| panic!("host chooser must be drawn"));
    cx.simulate_click(row.center(), gpui::Modifiers::none());
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.shell
                .herdr2
                .new_space
                .as_ref()
                .and_then(Chooser::selection),
            Some(("dev@studio.local", HerdrKind::Herdr2))
        );
    });
}

const HOST: &str = "sandbox";

fn choose(view: &gpui::Entity<Smoke>, cx: &mut gpui::VisualTestContext, kind: HerdrKind) {
    view.update(cx, |this, cx| {
        this.open_new_space(cx);
        let chooser = this
            .shell
            .herdr2
            .new_space
            .as_mut()
            .unwrap_or_else(|| panic!("chooser must open"));
        chooser.hosts = vec![make_host(
            HostName::new(HOST),
            vec![HerdrKind::Herdr1, HerdrKind::Herdr2],
        )];
        chooser.select_host(0);
        chooser.select_kind(kind);
    });
    cx.run_until_parked();
}

fn click(cx: &mut gpui::VisualTestContext, name: &'static str) {
    let center = cx
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .center();
    cx.simulate_click(center, gpui::Modifiers::none());
}

fn wait(
    view: &gpui::Entity<Smoke>,
    cx: &mut gpui::VisualTestContext,
    condition: impl Fn(&Smoke) -> bool,
) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        condition(this)
    }) {
        assert!(std::time::Instant::now() < until, "peer did not settle");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn open_herdr1(
    view: &gpui::Entity<Smoke>,
    cx: &mut gpui::VisualTestContext,
) -> std::sync::mpsc::Receiver<sheprd_client::protocol::ClientMessage> {
    let (client, receipts) = crate::mock_peer::connect_recorded(1)
        .unwrap_or_else(|error| panic!("Herdr 1 peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host(HOST);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait(view, cx, |this| this.terminal.live.surface.is_some());
    cx.run_until_parked();
    receipts
}

fn workspace_peer() -> crate::Result<(
    sheprd_client::Client,
    std::sync::mpsc::Receiver<sheprd_client::protocol::ClientMessage>,
)> {
    use sheprd_client::{
        ConnectOptions, ConnectTarget, Stream,
        protocol::{endpoint::*, *},
    };

    let (local, mut peer) = Stream::pair()?;
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(
        move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
            let _: ClientMessage = read_message(&mut peer, MAX_FRAME_SIZE)?;
            let mut welcome: serde_json::Value =
                serde_json::from_str(include_str!("../../../fixtures/endpoint-welcome-v1.json"))?;
            welcome["server_version"] = sheprd_client::MIN_HERDR_VERSION.into();
            welcome["methods"] = serde_json::json!(["workspace.create"]);
            for (kind, data) in [
                (ENDPOINT_WELCOME_KIND, welcome.to_string()),
                (
                    ENDPOINT_SNAPSHOT_KIND,
                    crate::mock_peer::SNAPSHOT.to_owned(),
                ),
            ] {
                write_message(
                    &mut peer,
                    &ServerMessage::EndpointControl {
                        kind: kind.into(),
                        data,
                    },
                    MAX_FRAME_SIZE,
                )?;
            }
            write_message(
                &mut peer,
                &ServerMessage::PaneSurface(crate::mock_peer::surface(7)),
                MAX_FRAME_SIZE,
            )?;
            loop {
                let message: ClientMessage = read_message(&mut peer, MAX_FRAME_SIZE)?;
                let done = matches!(message, ClientMessage::ClientShellEndpointRequest { .. });
                send.send(message)?;
                if done {
                    return Ok(());
                }
            }
        },
    );
    let client = sheprd_client::connect_with_connector(
        ConnectTarget::Local,
        ConnectOptions::default(),
        true,
        move |_, _| Ok(local),
    )
    .map_err(|error| crate::Error::Ssh(error.to_string()))?;
    Ok((client, receive))
}

#[gpui::test]
fn new_space_create_sends_herdr1_workspace_request(cx: &mut gpui::TestAppContext) {
    use sheprd_client::protocol::ClientMessage;

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) =
        workspace_peer().unwrap_or_else(|error| panic!("workspace peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host(HOST);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait(&view, cx, |this| {
        this.terminal.live.surface.is_some()
            && crate::actions::catalog(&this.terminal)
                .iter()
                .any(|row| row.available && row.info.command == sheprd_commands::Command::Workspace)
    });
    choose(&view, cx, HerdrKind::Herdr1);
    view.update(cx, |this, _| this.setup.busy = true);
    click(cx, "new-space-create-0");
    view.update(cx, |this, _| {
        assert!(this.new_space_open(), "setup must block Workspace creation");
        assert!(this.terminal.live.status.contains("host setup"));
        this.setup.busy = false;
    });
    click(cx, "new-space-create-0");
    view.read_with(cx, |this, _| assert!(!this.new_space_open()));
    loop {
        match receipts.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(ClientMessage::ClientShellEndpointRequest { request, .. }) => {
                let request: serde_json::Value = serde_json::from_str(&request)
                    .unwrap_or_else(|error| panic!("workspace request: {error}"));
                assert_eq!(request["method"], "workspace.create");
                break;
            }
            Ok(
                ClientMessage::ClientShellResize { .. } | ClientMessage::ClientShellFocus { .. },
            ) => {}
            message => panic!("expected workspace.create, received {message:?}"),
        }
    }
}

#[gpui::test]
fn new_space_create_uses_the_default_session_and_remembers_only_ready_session(
    cx: &mut gpui::TestAppContext,
) {
    // #144 decision 10 (AC10): Herdr 2's New space is a space in the
    // `default` core's document; no `space-<ms>` session is started.
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    choose(&view, cx, HerdrKind::Herdr2);
    let (mock, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("Herdr 2 peer: {error}"));
    view.update(cx, |this, _| this.terminal.core_test_backend = Some(mock));
    click(cx, "new-space-create-0");
    let name = view.read_with(cx, |this, _| {
        assert!(!this.new_space_open());
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("Create must start Herdr 2"))
            .name()
            .to_owned()
    });
    assert_eq!(name, "default", "no space-<ms> session");
    wait(&view, cx, |this| this.terminal.live.connected);
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        assert_eq!(this.shell.herdr2.selected_session(HOST), name);
        this.connect_named_core(HOST.into(), "failed".into(), None, cx)
            .unwrap_or_else(|error| panic!("worker start: {error}"));
        assert_eq!(this.shell.herdr2.selected_session(HOST), name);
    });
    wait(&view, cx, |this| {
        this.terminal.live.status.contains("/failed")
    });
    view.read_with(cx, |this, _| {
        assert_eq!(this.shell.herdr2.selected_session(HOST), name);
    });
}

#[gpui::test]
fn herdr2_new_space_writes_document_space(cx: &mut gpui::TestAppContext) {
    use crate::core::herdr_doc_sync::{DocPlan, DocSource};
    let host = crate::core_mock::Host::default();
    // Every terminal the document names is live: a space whose terminals
    // ended would be taken out of it (`core/ended_spaces.rs`).
    host.set_live(&["t1", "t2", "t30"]);
    host.doc().set_silent(
        serde_json::from_str(include_str!("../../../fixtures/herdr-app/two-spaces.json"))
            .unwrap_or_else(|error| panic!("fixture: {error}")),
    );
    let (backend, _) = host
        .recorded(false)
        .unwrap_or_else(|error| panic!("Herdr 2 peer: {error}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.select_host(HOST);
        this.terminal.reconnect.begin(HOST.into());
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core_mode = true;
        this.terminal.core = Some(
            crate::core::Session::start_doc(
                HOST.into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                None,
                crate::shell::single_pane::Mailbox::detached(),
                DocPlan {
                    source: DocSource::Core,
                    period: std::time::Duration::from_millis(200),
                },
            )
            .unwrap_or_else(|error| panic!("start: {error}")),
        );
    });
    wait(&view, cx, |this| !this.core_tabs().is_empty());
    choose(&view, cx, HerdrKind::Herdr2);
    click(cx, "new-space-create-0");
    let spaces = || host.doc().doc().0["spaces"].as_array().map_or(0, Vec::len);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while spaces() != 3 {
        assert!(std::time::Instant::now() < until, "no document space");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    view.read_with(cx, |this, _| {
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core"));
        assert_eq!(
            core.name(),
            "default",
            "the same session, not a space-<ms> one"
        );
    });
    assert_eq!(host.doc().sets().len(), 1);
}

#[gpui::test]
fn new_space_cancel_and_escape_do_not_connect(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let _peer = open_herdr1(&view, cx);
    for escape in [false, true] {
        choose(&view, cx, HerdrKind::Herdr2);
        if escape {
            view.update(cx, |this, cx| {
                this.terminal.needs_focus = true;
                cx.notify();
            });
            cx.run_until_parked();
            cx.simulate_keystrokes("escape");
        } else {
            click(cx, "new-space-cancel-0");
        }
        view.read_with(cx, |this, _| {
            assert!(!this.new_space_open());
            assert!(this.terminal.core.is_none(), "dismissal started a worker");
            assert_eq!(this.shell.herdr2.selected_session(HOST), "default");
        });
    }
}

#[gpui::test]
fn new_space_create_explains_why_connection_was_refused(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    choose(&view, cx, HerdrKind::Herdr2);
    view.update(cx, |this, _| this.terminal.reconnect.suspended = true);
    click(cx, "new-space-create-0");
    view.read_with(cx, |this, _| {
        assert!(this.terminal.core.is_none());
        assert!(this.terminal.live.status.contains("Resume the app"));
        assert!(
            this.shell
                .herdr2
                .new_space
                .as_ref()
                .and_then(|chooser| chooser.error.as_deref())
                .is_some_and(|error| error.contains("Resume the app"))
        );
    });
}

#[gpui::test]
fn new_space_keeps_focus_keys_and_paste_off_the_terminal_wire(cx: &mut gpui::TestAppContext) {
    use sheprd_backend::ClientPaneInputEvent;

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    let receipts = open_herdr1(&view, cx);
    choose(&view, cx, HerdrKind::Herdr1);
    view.update(cx, |this, cx| {
        this.terminal.input.fresh_gesture();
        this.terminal.needs_focus = true;
        cx.notify();
    });
    cx.run_until_parked();
    let terminal_focused =
        cx.update(|window, cx| view.update(cx, |this, _| this.terminal_focus.is_focused(window)));
    cx.simulate_keystrokes("up");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("must stay local".into()));
            this.native_menu_action(crate::edit_menu::EditAction::Paste, None, window, cx);
            assert!(this.new_space_open());
            this.terminal
                .link
                .handle
                .as_ref()
                .unwrap_or_else(|| panic!("live peer handle"))
                .send_input(
                    "boot-v1",
                    "w1:p1",
                    [ClientPaneInputEvent::Paste("modal FIFO fence".into())],
                )
                .unwrap_or_else(|error| panic!("queue peer fence: {error}"));
        });
    });
    let leaked = pane_input_before_fence(&receipts, "modal FIFO fence");
    assert!(
        leaked.is_empty(),
        "input leaked before FIFO fence: {leaked:?}"
    );
    assert!(
        !terminal_focused,
        "terminal reclaimed focus behind the chooser"
    );
}

fn pane_input_before_fence(
    receipts: &std::sync::mpsc::Receiver<sheprd_client::protocol::ClientMessage>,
    fence: &str,
) -> Vec<sheprd_backend::ClientPaneInputEvent> {
    use sheprd_backend::ClientPaneInputEvent;
    use sheprd_client::protocol::ClientMessage;

    let mut leaked = Vec::new();
    loop {
        match receipts.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(ClientMessage::ClientShellPaneInput { events, .. }) => {
                if events == [ClientPaneInputEvent::Paste(fence.into())] {
                    return leaked;
                }
                leaked.extend(events);
            }
            Ok(
                ClientMessage::ClientShellResize { .. } | ClientMessage::ClientShellFocus { .. },
            ) => {}
            message => panic!("expected FIFO fence, received {message:?}"),
        }
    }
}

#[gpui::test]
fn new_space_owns_keys_and_direct_paste_after_forced_terminal_focus(cx: &mut gpui::TestAppContext) {
    use crate::shell::footer::keys::KeyOwner;
    use sheprd_backend::ClientPaneInputEvent;

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    let receipts = open_herdr1(&view, cx);
    choose(&view, cx, HerdrKind::Herdr1);
    let (owner, owns_text, reads) = cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Exercise the shared input routes before the sheet draws again.
            this.terminal_focus.focus(window, cx);
            assert!(this.terminal_focus.is_focused(window));
            this.terminal.size = None;
            this.terminal.input.fresh_gesture();
            this.terminal_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("up")
                        .unwrap_or_else(|error| panic!("key fixture: {error}")),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("must stay local".into()));
            let before = this.handoff.clipboard_reads;
            this.start_paste(false, cx);
            this.flush_terminal();
            this.terminal
                .link
                .handle
                .as_ref()
                .unwrap_or_else(|| panic!("live peer handle"))
                .send_input(
                    "boot-v1",
                    "w1:p1",
                    [ClientPaneInputEvent::Paste(
                        "forced-focus FIFO fence".into(),
                    )],
                )
                .unwrap_or_else(|error| panic!("queue peer fence: {error}"));
            (
                this.key_owner(),
                this.terminal_owns_text_input(),
                this.handoff.clipboard_reads - before,
            )
        })
    });
    assert_eq!(
        reads, 0,
        "Paste read the terminal clipboard behind New space"
    );
    cx.run_until_parked();
    let leaked = pane_input_before_fence(&receipts, "forced-focus FIFO fence");
    assert!(
        leaked.is_empty(),
        "input reached the peer behind New space: {leaked:?}"
    );
    assert_eq!(owner, KeyOwner::NewSpaceSheet);
    assert!(!owns_text);
}

#[gpui::test]
fn new_space_blocks_direct_core_paste_before_the_peer_fence(cx: &mut gpui::TestAppContext) {
    use crate::{core::Command, terminal_input::destination};
    use sheprd_backend::ClientPaneInputEvent;

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (backend, receipts) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("Herdr 2 peer: {error}"));
    view.update(cx, |this, cx| {
        this.terminal.core_test_backend = Some(backend);
        this.connect_named_core(HOST.into(), "sandbox".into(), None, cx)
            .unwrap_or_else(|error| panic!("start Herdr 2: {error}"));
    });
    wait(&view, cx, |this| {
        destination(&this.terminal.live, this.terminal.link.generation).is_some()
    });
    choose(&view, cx, HerdrKind::Herdr2);
    view.update(cx, |this, cx| {
        let destination = destination(&this.terminal.live, this.terminal.link.generation)
            .unwrap_or_else(|| panic!("coherent peer destination"));
        // A stale UI callback may still carry the previously displayed address.
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("must stay local".into()));
        this.terminal.input.fresh_gesture();
        this.refresh_terminal_input();
        this.paste_core(destination.clone(), false, cx);
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core"));
        core.send(Command::Input(
            destination,
            ClientPaneInputEvent::Paste("core FIFO fence".into()),
        ))
        .unwrap_or_else(|error| panic!("queue peer fence: {error}"));
    });
    let (channel, bytes) = receipts
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap_or_else(|error| panic!("core peer fence: {error}"));
    assert_ne!(channel, 0);
    assert_eq!(
        bytes, b"core FIFO fence",
        "input leaked before the core FIFO fence"
    );
}
