use super::*;
use crate::{input_geometry::InputGeometry, mock_peer, terminal_painter::TerminalPainter};
use gpui::EntityInputHandler;
use gpui::{
    Bounds, Empty, KeyDownEvent, Keystroke, Point, TestAppContext, canvas, font, point, prelude::*,
    px, size,
};
use sheprd_client::{ClientEvent, protocol::*};
use std::sync::Arc;

fn live() -> Live {
    let mut live = Live::default();
    live.connected = true;
    let snapshot: ClientShellSnapshot =
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"));
    live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    live
}
fn ready() -> TerminalInput {
    let mut state = TerminalInput::default();
    state.sync(&live(), 1, None);
    state
}
fn key(name: &'static str, held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap_or_else(|e| panic!("key: {e}")),
        is_held: held,
        prefer_character_input: false,
    }
}
fn marked() -> TerminalInput {
    let mut input = ready();
    input.mark("a😀かな", Some(1..3));
    input
}
#[test]
fn emoji_utf16_selection_and_reads_do_not_split_a_scalar() {
    let mut state = marked();
    assert_eq!(state.composition.marked, Some(0..5));
    assert_eq!(state.composition.selected, 1..3);
    let start = state.composition.byte(1);
    let end = state.composition.byte(3);
    assert_eq!(&state.composition.text[start..end], "😀");
    state.mark("a😀b", Some(2..2));
    assert_eq!(
        state.composition.selected,
        3..3,
        "surrogate midpoint must snap to a scalar boundary"
    );
}
#[test]
fn remote_pane_change_discards_composition_and_late_commit() {
    let mut input = marked();
    let mut live = live();
    let mut snapshot = live
        .snapshot
        .as_deref()
        .cloned()
        .unwrap_or_else(|| panic!("snapshot"));
    snapshot.focused_pane_id = Some("w1:p2".into());
    live.snapshot = Some(Arc::new(snapshot));
    input.sync(&live, 1, None);
    assert!(
        input.composition.text.is_empty(),
        "old pane's marked text survived"
    );
    assert!(
        input.commit("旧変換").is_none(),
        "old pane's late commit leaked"
    );
    input.fresh_gesture();
    assert!(input.commit("新").is_some());
}
#[test]
fn popup_open_replace_and_close_each_discard_composition() {
    let mut live = live();
    let mut input = marked();
    for popup_id in [Some("popup-1"), Some("popup-2"), None] {
        let mut surface = live
            .surface
            .as_deref()
            .cloned()
            .unwrap_or_else(|| panic!("surface"));
        surface.popup = popup_id.map(|id| {
            Box::new(ClientShellPopupSurface {
                terminal_id: id.into(),
                title: "test".into(),
                width: None,
                height: None,
                frame: surface.frame.clone(),
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                pixel_width: 0,
                pixel_height: 0,
            })
        });
        live.surface = Some(Arc::new(surface));
        input.sync(&live, 1, None);
        assert!(
            input.composition.text.is_empty(),
            "popup boundary retained marked text"
        );
        assert!(input.commit("late").is_none());
        input.fresh_gesture();
        input.mark("next", Some(1..2));
    }
}
#[test]
fn disconnect_then_reconnect_does_not_replay_commit_or_held_keys() {
    let mut input = marked();
    let mut live = live();
    input.queue(ClientPaneInputEvent::TextCommit("queued".into()));
    live.apply(ClientEvent::Disconnected {
        reason: "test".into(),
        ssh: None,
    });
    input.sync(&live, 1, None);
    assert!(input.commit("old").is_none());
    input.sync(&self::live(), 2, None);
    assert!(input.next().is_none());
    assert!(input.commit("late").is_none());
}
#[test]
fn setup_focus_never_accepts_terminal_text() {
    let input = ready();
    assert!(input.accepts(true, false));
    assert!(!input.accepts(true, true), "setup owns text input");
    assert!(
        !input.accepts(false, false),
        "unfocused terminal took input"
    );
}
#[test]
fn printable_key_and_commit_arrive_once_with_repeat_and_release() {
    let mut input = ready();
    assert!(input.key_down(&key("a", false), true).is_none());
    assert_eq!(
        input.commit("a"),
        Some(ClientPaneInputEvent::TextCommit("a".into()))
    );
    for (held, kind) in [(false, ClientKeyKind::Press), (true, ClientKeyKind::Repeat)] {
        let event = input.key_down(&key("ctrl-c", held), true);
        assert!(
            matches!(event, Some(ClientPaneInputEvent::Key {kind: k, tracks_release: true, ..}) if k == kind)
        );
    }
    assert!(matches!(
        input.held.release("c"),
        Some(ClientPaneInputEvent::Key {
            kind: ClientKeyKind::Release,
            modifiers: 2,
            ..
        })
    ));
    assert!(input.held.release("c").is_none());
    input.key_down(&key("left", false), false);
    assert!(input.held.release("left").is_none());
}
#[test]
fn option_command_and_escape_variants_change_semantics() {
    let mut input = ready();
    assert!(input.key_down(&key("alt-p", false), false).is_none());
    input.options.option_alt = true;
    assert!(input.key_down(&key("alt-p", false), false).is_some());
    assert!(input.key_down(&key("cmd-c", false), false).is_none());
    input.options.command_control = true;
    assert!(matches!(
        input.key_down(&key("cmd-c", false), false),
        Some(ClientPaneInputEvent::Key { modifiers: 2, .. })
    ));
    input.mark("かな", Some(0..1));
    assert!(input.key_down(&key("escape", false), false).is_none());
    assert_eq!(input.composition.text, "かな");
    input.options.escape_cancels = true;
    assert!(matches!(
        input.key_down(&key("escape", false), false),
        Some(ClientPaneInputEvent::Key {
            code: ClientKeyCode::Esc,
            ..
        })
    ));
    assert!(input.composition.text.is_empty());
}
#[test]
fn keyboard_resize_keeps_composition_and_fifo_only_for_same_target() {
    let mut input = marked();
    let live = live();
    let options = ConnectOptions {
        surface_size: ClientSurfaceSize { cols: 60, rows: 15 },
        ..Default::default()
    };
    input.sync(&live, 1, Some(options));
    assert!(!input.ready);
    assert!(
        input.accepts(true, false),
        "resize must not resign keyboard"
    );
    assert_eq!(input.composition.text, "a😀かな");
    assert_eq!(input.composition.selected, 1..3);
    input.queue(ClientPaneInputEvent::TextCommit("first".into()));
    input.queue(ClientPaneInputEvent::TextCommit("second".into()));
    assert!(input.next().is_none());
    input.sync(&live, 1, None);
    assert_eq!(
        input.next(),
        Some(ClientPaneInputEvent::TextCommit("first".into()))
    );
    assert_eq!(
        input.next(),
        Some(ClientPaneInputEvent::TextCommit("second".into()))
    );
    for _ in 0..64 {
        assert!(input.queue(ClientPaneInputEvent::TextCommit("x".into())));
    }
    assert!(!input.queue(ClientPaneInputEvent::TextCommit("overflow".into())));
}
#[gpui::test]
fn mid_clause_rotation_keyboard_resize_share_paint_hit_and_candidate_geometry(
    cx: &mut TestAppContext,
) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    cx.draw(Point::default(), size(px(900.), px(900.)), |_, _| {
        canvas(
            |_, _, _| (),
            |_, _, window, cx| {
                let mut painter = TerminalPainter::default();
                let font = font("Menlo");
                let cell_width = painter.cell_width(&font, window, cx);
                let mut input = ready();
                input.mark("kanakana", Some(2..4));
                for (width, height) in [(120., 300.), (260., 140.), (120., 100.)] {
                    let geometry = InputGeometry {
                        grid: Bounds::new(point(px(20.), px(80.)), size(px(width), px(height))),
                        cell_width,
                        cell_height: 20.,
                    };
                    let mut surface = mock_peer::surface(7);
                    let col = (width / cell_width).floor() as u16 - 1;
                    let row = (height / 20.).floor() as u16 - 1;
                    surface.frame.cursor = Some(CursorState {
                        x: col,
                        y: row,
                        visible: true,
                        shape: 1,
                    });
                    let cursor = geometry.cursor(Some(&surface));
                    let area = geometry.area(Some(&surface));
                    painter.paint_composition(
                        &input.composition.text,
                        cursor.origin,
                        area,
                        &font,
                        window,
                    );
                    let clause = painter.composition_bounds(
                        &input.composition.text,
                        input.composition.selected.clone(),
                        cursor,
                        area,
                        &font,
                        window,
                    );
                    let end = painter.composition_bounds(
                        &input.composition.text,
                        8..8,
                        cursor,
                        area,
                        &font,
                        window,
                    );
                    assert!(
                        clause.left() < cursor.left(),
                        "candidate must follow mid-clause, not cursor cell"
                    );
                    assert!(
                        clause.left() < end.left(),
                        "candidate must not jump to composition end"
                    );
                    assert_eq!(
                        painter.composition_index(
                            &input.composition.text,
                            clause.origin,
                            cursor,
                            area,
                            &font,
                            window
                        ),
                        Some(2),
                        "paint and hit-test differ"
                    );
                    assert!(geometry.contains(clause.origin));
                    assert_eq!(
                        clause.top(),
                        px(80. + f32::from(row) * 20.),
                        "rotation/keyboard resize must move candidate with painted row"
                    );
                    assert_eq!(
                        end.right(),
                        geometry.grid.right(),
                        "rotation/keyboard resize must use the new right edge"
                    );
                    assert_eq!(input.composition.selected, 2..4);
                }
            },
        )
        .size_full()
    });
}

#[test]
fn receiving_peer_gets_one_semantic_commit_and_rejects_after_disconnect() {
    use std::time::{Duration, Instant};
    let (client, received) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    let mut link = crate::link::Link::default();
    let generation = link.reset();
    link.install(generation, Ok(client));
    let deadline = Instant::now() + Duration::from_secs(5);
    let target = loop {
        if let Ok(live) = link.inbox.try_lock()
            && let Some(target) = destination(&live, generation)
        {
            break target;
        }
        assert!(
            Instant::now() < deadline,
            "peer never supplied coherent target"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    link.send_verified(&target, ClientPaneInputEvent::TextCommit("一😀".into()))
        .unwrap_or_else(|e| panic!("send: {e}"));
    let message = received
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|e| panic!("receipt: {e}"));
    assert!(
        matches!(message,ClientMessage::ClientShellPaneInput{pane_id,events} if pane_id == "w1:p1" && events == vec![ClientPaneInputEvent::TextCommit("一😀".into())])
    );
    link.inbox
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .apply(ClientEvent::Disconnected {
            reason: "test".into(),
            ssh: None,
        });
    assert!(
        link.send_verified(&target, ClientPaneInputEvent::TextCommit("late".into()))
            .is_err(),
        "stale UI committed after reader disconnected"
    );
    assert!(
        received.recv_timeout(Duration::from_millis(100)).is_err(),
        "late or duplicate wire input"
    );
}

#[gpui::test]
fn uikit_handler_callbacks_preserve_mark_selection_and_setup_isolation(cx: &mut TestAppContext) {
    use crate::app::Smoke;
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        this.terminal.open = true;
        this.terminal.live = live();
        *this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = live();
        this.setup.busy = false;
        cx.notify();
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.replace_and_mark_text_in_range(None, "a😀かな", Some(1..3), window, cx);
            assert_eq!(
                this.selected_text_range(false, window, cx).map(|s| s.range),
                Some(1..3)
            );
            assert_eq!(this.marked_text_range(window, cx), Some(0..5));
            let mut adjusted = None;
            assert_eq!(
                this.text_for_range(1..3, &mut adjusted, window, cx)
                    .as_deref(),
                Some("😀")
            );
            assert_eq!(adjusted, Some(1..3));
            this.set_selected_text_range(3..4, window, cx);
            assert_eq!(
                this.selected_text_range(false, window, cx).map(|s| s.range),
                Some(3..4)
            );
            this.terminal.open = false;
            this.replace_text_in_range(None, "stale terminal callback", window, cx);
            assert_eq!(this.input.text, "", "terminal focus leaked into setup");
            this.focus.focus(window, cx);
            this.replace_text_in_range(None, "user@host", window, cx);
            assert_eq!(this.input.text, "user@host");
            assert_eq!(
                this.terminal.input.composition.text, "a😀かな",
                "setup edited terminal composition"
            );
            this.setup.busy = true;
            assert!(!this.accepts_text_input(window, cx));
            this.replace_text_in_range(None, "blocked", window, cx);
            assert_eq!(this.input.text, "user@host");
        });
    });
}

#[test]
fn mailbox_cannot_hide_a_target_switch_away_and_back() {
    let mut live = live();
    let mut input = TerminalInput::default();
    input.sync(&live, 1, None);
    input.mark("old composition", Some(1..2));
    let original = live
        .snapshot
        .as_deref()
        .cloned()
        .unwrap_or_else(|| panic!("snapshot"));
    let mut other = original.clone();
    other.revision += 1;
    other.focused_pane_id = Some("w1:p2".into());
    live.apply(ClientEvent::Snapshot(Arc::new(other.clone())));
    let mut frame = mock_peer::surface(other.revision);
    live.apply(ClientEvent::Surface(Arc::new(frame.clone())));
    other.focused_pane_id = original.focused_pane_id;
    other.revision += 1;
    frame.projection_revision = other.revision;
    live.apply(ClientEvent::Snapshot(Arc::new(other)));
    live.apply(ClientEvent::Surface(Arc::new(frame)));
    input.sync(&live, 1, None);
    assert!(
        input.composition.text.is_empty(),
        "mailbox conflated away a target change"
    );
    assert!(input.commit("late").is_none());
}

#[test]
fn popup_paint_and_candidate_use_the_same_center_and_cursor() {
    let mut surface = mock_peer::surface(7);
    let mut popup = surface.frame.clone();
    popup.width = 20;
    popup.height = 6;
    popup.cursor = Some(CursorState {
        x: 2,
        y: 3,
        visible: true,
        shape: 1,
    });
    surface.popup = Some(Box::new(ClientShellPopupSurface {
        terminal_id: "popup".into(),
        title: "test".into(),
        width: None,
        height: None,
        frame: popup,
        mouse_reporting: false,
        sgr_pixel_mouse: false,
        pixel_width: 0,
        pixel_height: 0,
    }));
    let geometry = InputGeometry {
        grid: Bounds::new(point(px(20.), px(80.)), size(px(800.), px(480.))),
        cell_width: 10.,
        cell_height: 20.,
    };
    let area = geometry.area(Some(&surface));
    assert_eq!(
        area,
        Bounds::new(point(px(320.), px(260.)), size(px(200.), px(120.)))
    );
    assert_eq!(
        geometry.cursor(Some(&surface)),
        Bounds::new(point(px(340.), px(320.)), size(px(10.), px(20.)))
    );
}
