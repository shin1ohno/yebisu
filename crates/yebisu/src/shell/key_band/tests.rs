use super::*;
use crate::{
    layout::{Rect, Size},
    mock_peer,
    shell::{add_host::SheetMode, fit},
    terminal::InputTarget,
};
use gpui::{
    AnyWindowHandle, App, ClipboardItem, Edges, EntityInputHandler, Global, TestAppContext,
    VisualTestContext, WindowInsets, size,
};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Dismissals(std::cell::RefCell<Vec<AnyWindowHandle>>);

impl Global for Dismissals {}

// GPUI's test platform keeps its dismissal counter private to the crate.
pub(super) fn observe_dismissal(dismiss: fn(&Window), cx: &App) -> impl FnOnce(&Window) + '_ {
    move |window| {
        dismiss(window);
        if let Some(dismissals) = cx.try_global::<Dismissals>() {
            dismissals.0.borrow_mut().push(window.window_handle());
        }
    }
}

fn record_dismissals(cx: &mut VisualTestContext) -> AnyWindowHandle {
    cx.update(|window, app| {
        app.set_global(Dismissals::default());
        window.window_handle()
    })
}

fn assert_dismissals(cx: &mut VisualTestContext, expected: &[AnyWindowHandle]) {
    cx.update(|_, app| assert_eq!(app.global::<Dismissals>().0.borrow().as_slice(), expected));
}

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

#[test]
fn a_committed_control_character_consumes_the_latch_without_reencoding() {
    for (modifier, text) in [(Action::Control, "\n"), (Action::Alt, "\t")] {
        let mut state = State::default();
        state.toggle(modifier, destination());
        assert_eq!(
            apply_latch(&mut state, commit(text), true),
            [Some(commit(text)), None]
        );
        assert_eq!(
            apply_latch(&mut state, commit("a"), true),
            [Some(commit("a")), None]
        );
        assert!(!state.is_armed());
    }
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

    tap(cx, "key-band-ctrl");
    cx.update(|window, app| {
        view.update(app, |app, cx| {
            app.replace_text_in_range(None, "\n", window, cx);
        });
    });
    assert_eq!(received(&replies), commit("\n").event);
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
    let window = record_dismissals(cx);
    tap(cx, "key-band-hide");
    assert_dismissals(cx, &[window]);
    assert!(!view.update(cx, |app, _| app.shell.frame.key_band.is_armed()));
}

#[gpui::test]
fn hide_dismisses_the_keyboard_over_settings(cx: &mut TestAppContext) {
    hide_over_sheet(cx, "settings-overlay", |app, window, cx| {
        app.begin_settings_modal(window, cx);
    });
}

#[gpui::test]
fn hide_dismisses_the_keyboard_over_add_host(cx: &mut TestAppContext) {
    hide_over_sheet(cx, "hosts-overlay", |app, window, cx| {
        app.open_hosts(cx);
        app.shell.hosts_sheet.open_add(SheetMode::Add, "");
        app.hand_keys_to_sheet(window, cx);
    });
}

fn hide_over_sheet(
    cx: &mut TestAppContext,
    overlay: &'static str,
    open: impl FnOnce(&mut Smoke, &mut Window, &mut Context<Smoke>),
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    let (client, _) = mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    wait_for_terminal(&view, cx);
    let window = record_dismissals(cx);
    cx.update(|window, app| view.update(app, |app, cx| open(app, window, cx)));
    set_insets(&view, cx, 498., false);
    let scrim = cx
        .debug_bounds(overlay)
        .unwrap_or_else(|| panic!("{overlay} is absent"));
    let hide = cx
        .debug_bounds("key-band-hide")
        .unwrap_or_else(|| panic!("Hide is absent"));
    assert!(scrim.contains(&hide.center()));
    cx.update(|window, app| {
        view.update(app, |app, cx| {
            assert!(app.input_modal());
            app.shell
                .frame
                .key_band
                .toggle(Action::Control, destination());
            app.press_key_band(Action::Key("tab"), window, cx);
            assert!(app.shell.frame.key_band.is_armed());
            app.press_key_band(Action::Paste, window, cx);
            assert!(app.shell.frame.key_band.is_armed());
        });
    });
    assert_dismissals(cx, &[]);
    tap(cx, "key-band-hide");
    assert_dismissals(cx, &[window]);
    view.update(cx, |app, _| {
        assert!(app.input_modal());
        assert!(!app.shell.frame.key_band.is_armed());
    });
}

/// One finger pan step of `x`, `y` points over `at` (issue #137 PR 2).
fn swipe(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, x: f32, y: f32) {
    cx.simulate_event(gpui::ScrollEvent {
        position: at,
        delta: gpui::ScrollDelta::Pixels(point(px(x), px(y))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Started,
        source: gpui::ScrollSource::Touch,
    });
    cx.run_until_parked();
}

/// Issue #137 PR 2: the overflowing key band moves sideways under a
/// sideways swipe only; a vertical swipe leaves it where it is.
#[gpui::test]
fn the_key_band_ignores_vertical_swipes(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(320.), px(834.)));
    let (client, _) = mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    wait_for_terminal(&view, cx);
    set_insets(&view, cx, 320., false);
    wait_for_terminal(&view, cx);
    let esc = |cx: &mut VisualTestContext| {
        cx.debug_bounds("key-band-esc")
            .unwrap_or_else(|| panic!("esc key"))
    };
    let at = esc(cx).center();
    let start = esc(cx).left();
    swipe(cx, at, 0., -80.);
    assert_eq!(esc(cx).left(), start, "a vertical swipe moved the band");
    swipe(cx, at, -80., 0.);
    assert!(
        esc(cx).left() < start,
        "a sideways swipe must move the overflowing band"
    );
}

/// While the native search panel is up (issue #138), a band press that began
/// on the old layout changes nothing: Hide dismisses no keyboard and keeps
/// the latch, and a key sends no input.
#[gpui::test]
fn band_presses_while_search_is_open_change_nothing(cx: &mut TestAppContext) {
    use crate::shell::search::{SearchEvent, tests::fake::RecordedSearch};
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    let (client, replies) =
        mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    wait_for_terminal(&view, cx);
    set_insets(&view, cx, 498., false);
    cx.update(|window, app| {
        view.update(app, |view, cx| view.terminal_focus.focus(window, cx));
    });
    cx.run_until_parked();
    let fake = RecordedSearch::default();
    let presenter = fake.clone();
    view.update(cx, |app, _| {
        app.shell.search.set_presenter(Box::new(presenter))
    });
    record_dismissals(cx);
    cx.update(|window, app| {
        view.update(app, |app, cx| {
            let anchor = app.search_anchor(point(px(1.), px(1.)));
            app.open_sidebar_search(anchor, window, cx);
        });
    });
    fake.push(SearchEvent::Opened);
    cx.run_until_parked();
    assert!(
        view.update(cx, |app, _| app.search_open()),
        "fixture: the search panel is up"
    );
    cx.update(|window, app| {
        view.update(app, |app, cx| {
            app.shell
                .frame
                .key_band
                .toggle(Action::Control, destination());
            app.press_key_band(Action::Hide, window, cx);
            assert!(
                app.shell.frame.key_band.is_armed(),
                "Hide acted while the panel was up"
            );
            app.press_key_band(Action::Key("escape"), window, cx);
        });
    });
    cx.run_until_parked();
    assert_dismissals(cx, &[]);
    let deadline = Instant::now() + Duration::from_millis(300);
    while let Ok(message) = replies.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    {
        assert!(
            !matches!(message, ClientMessage::ClientShellPaneInput { .. }),
            "a band key reached the terminal while the panel was up: {message:?}"
        );
    }
}
