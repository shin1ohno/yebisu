use crate::{app::Smoke, core::Command, shell::HerdrKind, terminal_input::destination};
use gpui::{
    Entity, KeyDownEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, px, size,
};
use sheprd_backend::ClientPaneInputEvent;
use sheprd_client::protocol::ClientMessage;
use std::time::{Duration, Instant};

const FENCE: &str = "rail chooser FIFO fence";

fn open_from_rail(view: &Entity<Smoke>, cx: &mut VisualTestContext, kind: HerdrKind) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        destination(&this.terminal.live, this.terminal.link.generation).is_some()
    }) {
        assert!(Instant::now() < deadline, "{kind:?} peer did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.simulate_resize(size(px(1376.), px(1032.)));
    cx.update(|window, _| window.activate_window());
    view.update(cx, |this, cx| {
        this.actions.board_hidden = true;
        assert_eq!(this.terminal.herdr_kind(), kind);
        cx.notify();
    });
    cx.run_until_parked();
    let plus = cx
        .debug_bounds("rail-item-0")
        .unwrap_or_else(|| panic!("{kind:?}: rail New space button missing"));
    cx.simulate_click(plus.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |this, _| this.new_space_open()),
        "{kind:?}: rail New space button did not open the chooser"
    );
    assert!(cx.debug_bounds("new-space-host-0").is_some());
}

fn try_terminal_input(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Exercise a terminal callback even if a focus change races the chooser.
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.terminal.input.fresh_gesture();
            this.terminal_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("up")
                        .unwrap_or_else(|error| panic!("key fixture: {error}")),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("must stay local".into()));
            let reads = this.handoff.clipboard_reads;
            this.start_paste(false, cx);
            assert_eq!(this.handoff.clipboard_reads, reads);
            this.flush_terminal();
        });
    });
}

#[gpui::test]
fn rail_new_space_blocks_keys_and_paste_on_herdr1(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::mock_peer::connect_recorded(1)
        .unwrap_or_else(|error| panic!("Herdr 1 peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    open_from_rail(&view, cx, HerdrKind::Herdr1);
    try_terminal_input(&view, cx);
    view.read_with(cx, |this, _| {
        this.terminal
            .link
            .handle
            .as_ref()
            .unwrap_or_else(|| panic!("live peer handle"))
            .send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste(FENCE.into())],
            )
            .unwrap_or_else(|error| panic!("queue peer fence: {error}"));
    });
    loop {
        match receipts.recv_timeout(Duration::from_secs(3)) {
            Ok(ClientMessage::ClientShellPaneInput { events, .. }) => {
                assert_eq!(events, [ClientPaneInputEvent::Paste(FENCE.into())]);
                break;
            }
            Ok(
                ClientMessage::ClientShellResize { .. } | ClientMessage::ClientShellFocus { .. },
            ) => {}
            message => panic!("expected peer fence, received {message:?}"),
        }
    }
}

#[gpui::test]
fn rail_new_space_blocks_keys_and_paste_on_herdr2(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (backend, receipts) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("Herdr 2 peer: {error}"));
    view.update(cx, |this, cx| {
        this.terminal.core_test_backend = Some(backend);
        this.connect_named_core("sandbox".into(), "sandbox".into(), None, cx)
            .unwrap_or_else(|error| panic!("start Herdr 2: {error}"));
    });
    open_from_rail(&view, cx, HerdrKind::Herdr2);
    try_terminal_input(&view, cx);
    view.read_with(cx, |this, _| {
        let destination = destination(&this.terminal.live, this.terminal.link.generation)
            .unwrap_or_else(|| panic!("coherent peer destination"));
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("live core handle"))
            .send(Command::Input(
                destination,
                ClientPaneInputEvent::Paste(FENCE.into()),
            ))
            .unwrap_or_else(|error| panic!("queue peer fence: {error}"));
    });
    let (channel, bytes) = receipts
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or_else(|error| panic!("core peer fence: {error}"));
    assert_ne!(channel, 0);
    assert_eq!(bytes, FENCE.as_bytes());
}
