use super::*;
use crate::{
    layout::{Rect, Size},
    mock_peer,
    shell::fit,
    terminal::InputTarget,
};
use gpui::{ClipboardItem, Edges, TestAppContext, VisualTestContext, WindowInsets, size};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::time::{Duration, Instant};

fn destination() -> Destination {
    Destination {
        generation: 1,
        epoch: 0,
        boot: "boot".into(),
        target: InputTarget::Pane("w1:p1".into()),
    }
}

fn commit(text: &str) -> GeneratedInput {
    GeneratedInput {
        destination: destination(),
        event: ClientPaneInputEvent::TextCommit(text.into()),
    }
}

fn down(code: ClientKeyCode, modifiers: u8) -> ClientPaneInputEvent {
    ClientPaneInputEvent::Key {
        code,
        modifiers,
        kind: ClientKeyKind::Press,
        repeat_count: 1,
        shifted_codepoint: None,
        generated_text: None,
        tracks_release: false,
        physical_key_id: None,
        windows_record: None,
    }
}

#[test]
fn latch_applies_to_one_scalar_and_emits_a_balanced_report_all_pair() {
    let mut state = State::default();
    for action in [Action::Control, Action::Alt] {
        state.toggle(action, destination());
        assert!(state.pressed(action, Some(&destination())));
    }
    assert_eq!(
        apply_latch(&mut state, commit("かな"), true),
        [Some(commit("かな")), None]
    );
    assert!(state.is_armed());
    let [Some(press), Some(release)] = apply_latch(&mut state, commit("a"), true) else {
        panic!("both keyboard events should reach the same destination");
    };
    for (event, expected_kind) in [
        (press, ClientKeyKind::Press),
        (release, ClientKeyKind::Release),
    ] {
        assert!(matches!(event.event, ClientPaneInputEvent::Key {
            code: ClientKeyCode::Char('a'), modifiers: 6, kind, tracks_release: true, ..
        } if kind == expected_kind));
        assert_eq!(event.destination, destination());
    }
    assert!(!state.is_armed());
    assert_eq!(
        apply_latch(&mut state, commit("b"), false),
        [Some(commit("b")), None]
    );
}

#[test]
fn toggling_twice_and_switching_destination_both_clear_the_latch() {
    let mut state = State::default();
    state.toggle(Action::Alt, destination());
    state.toggle(Action::Alt, destination());
    assert!(!state.is_armed());
    let mut other = destination();
    other.epoch += 1;
    state.toggle(Action::Control, other);
    assert_eq!(
        apply_latch(&mut state, commit("x"), false),
        [Some(commit("x")), None]
    );
    assert!(!state.is_armed());
}

fn insets(keyboard: f32, left: f32) -> WindowInsets {
    WindowInsets {
        ime: Edges {
            bottom: px(keyboard),
            left: px(left),
            ..Edges::default()
        },
        ..WindowInsets::default()
    }
}

#[test]
fn band_is_reserved_only_for_a_docked_software_keyboard() {
    let window = Size::new(1376., 1032.);
    for (height, left, attached, expected) in [
        (498., 0., false, 76.),
        (498., 0., true, 0.),
        (498., 420., false, 0.),
        (71., 0., false, 0.),
        (25., 0., false, 0.),
        (0., 0., false, 0.),
    ] {
        assert_eq!(
            fit::key_band_height(window, &insets(height, left).ime, attached),
            expected
        );
    }
}

fn install(app: &mut Smoke, client: sheprd_client::Client) {
    app.select_host("key-band-peer");
    let generation = app.terminal.link.reset();
    app.terminal.link.install(generation, Ok(client));
    app.terminal.open = true;
}

fn wait_for_terminal(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |app, cx| {
        app.poll_terminal(cx);
        app.refresh_terminal_input();
        app.terminal.input.ready && !app.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
}

fn set_insets(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext, height: f32, attached: bool) {
    view.update(cx, |app, cx| {
        app.shell.fit.insets = Some(insets(height, 0.));
        app.shell.fit.hardware_keyboard_attached = attached;
        cx.notify();
    });
    cx.run_until_parked();
}

#[gpui::test]
fn rendered_band_and_terminal_fit_share_the_guide(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, _) = mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    for (width, height, keyboard) in [(1376., 1032., 498.), (1210., 834., 320.)] {
        cx.simulate_resize(size(px(width), px(height)));
        wait_for_terminal(&view, cx);
        set_insets(&view, cx, 0., false);
        wait_for_terminal(&view, cx);
        let bare_size = view
            .update(cx, |app, _| app.terminal.size)
            .unwrap_or_else(|| panic!("size before keyboard"))
            .surface_size;
        let starting = cx
            .debug_bounds("shell-terminal")
            .unwrap_or_else(|| panic!("terminal"));
        assert!(cx.debug_bounds("shell-key-band").is_none());
        assert!(cx.debug_bounds("shell-status-shortcuts").is_some());
        set_insets(&view, cx, keyboard, false);
        assert!(cx.debug_bounds("shell-status-software-keyboard").is_some());
        assert!(cx.debug_bounds("shell-status-shortcuts").is_none());
        wait_for_terminal(&view, cx);
        let band = cx
            .debug_bounds("shell-key-band")
            .unwrap_or_else(|| panic!("missing band"));
        let terminal = cx
            .debug_bounds("shell-terminal")
            .unwrap_or_else(|| panic!("terminal"));
        assert_eq!(band.left(), px(0.));
        assert_eq!(band.size, size(px(width), px(76.)));
        assert_eq!(band.bottom(), px(height - keyboard));
        assert_eq!(terminal.size.width, starting.size.width);
        let with_band = view
            .update(cx, |app, _| app.terminal.size)
            .unwrap_or_else(|| panic!("size with keyboard"))
            .surface_size;
        assert_eq!(with_band.cols, bare_size.cols);
        assert!(with_band.rows < bare_size.rows);
        assert_eq!(
            starting.size.height - terminal.size.height,
            px(keyboard + 76.)
        );
        view.update(cx, |app, _| {
            let layout = app.layout_for(
                Size::new(width, height),
                &insets(keyboard, 0.),
                Default::default(),
            );
            assert_eq!(
                layout.key_band,
                Some(Rect::new(0., height - keyboard - 76., width, 76.))
            );
            assert_eq!(layout.status_bar.bottom(), height - keyboard - 76.);
            assert!(
                layout
                    .terminal
                    .rect()
                    .is_some_and(|rect| rect.bottom() <= layout.status_bar.y)
            );
        });
        set_insets(&view, cx, keyboard, true);
        assert!(cx.debug_bounds("shell-key-band").is_none());
        set_insets(&view, cx, 71., false);
        assert!(cx.debug_bounds("shell-key-band").is_none());
    }
}

fn tap(cx: &mut VisualTestContext, id: &'static str) {
    let bounds = cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("{id} is absent"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

fn received(replies: &std::sync::mpsc::Receiver<ClientMessage>) -> ClientPaneInputEvent {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let incoming = replies.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        match incoming {
            Ok(ClientMessage::ClientShellPaneInput { events, .. }) => {
                assert_eq!(events.len(), 1);
                return events
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| panic!("empty input"));
            }
            Ok(_) => continue,
            Err(error) => panic!("peer did not receive the key: {error}"),
        }
    }
}

#[gpui::test]
fn taps_use_semantic_input_on_the_current_peer(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    let (client, replies) =
        mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    wait_for_terminal(&view, cx);
    set_insets(&view, cx, 498., false);
    wait_for_terminal(&view, cx);
    cx.update(|window, app| {
        view.update(app, |view, cx| view.terminal_focus.focus(window, cx));
    });
    cx.run_until_parked();
    for (id, code) in [
        ("key-band-esc", ClientKeyCode::Esc),
        ("key-band-tab", ClientKeyCode::Tab),
        ("key-band-left", ClientKeyCode::Left),
        ("key-band-up", ClientKeyCode::Up),
        ("key-band-down", ClientKeyCode::Down),
        ("key-band-right", ClientKeyCode::Right),
    ] {
        tap(cx, id);
        assert_eq!(received(&replies), down(code, 0), "{id}");
    }
    for (id, text) in [
        ("key-band-pipe", "|"),
        ("key-band-tilde", "~"),
        ("key-band-slash", "/"),
        ("key-band-minus", "-"),
    ] {
        tap(cx, id);
        assert_eq!(received(&replies), commit(text).event, "{id}");
    }
    tap(cx, "key-band-ctrl");
    tap(cx, "key-band-alt");
    tap(cx, "key-band-slash");
    assert_eq!(received(&replies), down(ClientKeyCode::Char('/'), 6));
    tap(cx, "key-band-minus");
    assert_eq!(received(&replies), commit("-").event);

    tap(cx, "key-band-ctrl");
    cx.simulate_input("c");
    assert_eq!(received(&replies), down(ClientKeyCode::Char('c'), 2));
    cx.simulate_input("a");
    assert_eq!(received(&replies), commit("a").event);

    cx.write_to_clipboard(ClipboardItem::new_string("key band paste".into()));
    tap(cx, "key-band-paste");
    cx.run_until_parked();
    assert_eq!(
        received(&replies),
        ClientPaneInputEvent::Paste("key band paste".into())
    );

    tap(cx, "key-band-alt");
    assert!(view.update(cx, |app, _| app.shell.frame.key_band.is_armed()));
    tap(cx, "key-band-hide");
    assert!(!view.update(cx, |app, _| app.shell.frame.key_band.is_armed()));
}
