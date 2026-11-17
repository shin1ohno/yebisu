//! SH1-156 device failure (P13 owner check steps 7-13 and 21-25) as the host
//! sees it: after a discarded composition, a tap or pointer click on the
//! terminal and the Select control must let new letters reach the terminal.
//! The pointer goes through GPUI's real mouse dispatch to the terminal element
//! (terminal_view.rs), keys and text through the real adapters. These prove app
//! routing only, not physical UIKit delivery or native first-responder state.
use crate::{app::Smoke, mock_peer};
use gpui::{
    Entity, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    TestAppContext, VisualTestContext, point, px, size,
};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const END: &str = "SH1-156-tap-end";

fn connected(
    cx: &mut TestAppContext,
) -> (
    Entity<Smoke>,
    &mut VisualTestContext,
    mpsc::Receiver<ClientMessage>,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some() && this.handoff.stamp.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "SH1-156 frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx, wire)
}

/// A composition the user started, then lost (the device showed
/// "Composition discarded · tap terminal to type").
fn discard_composition(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_and_mark_text_in_range(None, "てすと", Some(0..3), window, cx);
            assert_eq!(this.terminal.input.composition.text, "てすと");
            this.terminal.input.discard();
            assert!(this.terminal.input.requires_gesture(), "fixture discards");
            assert!(this.handoff.drag_select, "Start: Drag is the default");
        })
    });
    cx.run_until_parked();
}

fn terminal_point(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let g = view.update(cx, |this, _| this.terminal.geometry);
    assert!(g.grid.size.height > px(60.), "terminal must be laid out");
    g.grid.origin + point(px(3.5 * g.cell_width), px(2.5 * g.cell_height))
}

/// One press and release on the same cell: a finger tap, or a trackpad click
/// after the pointer moved there.
fn tap(view: &Entity<Smoke>, cx: &mut VisualTestContext, hover: bool) {
    let at = terminal_point(view, cx);
    if hover {
        cx.simulate_mouse_move(at, None, Modifiers::default());
    }
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn press(view: &Entity<Smoke>, cx: &mut VisualTestContext, key: &'static str) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let keystroke = Keystroke::parse(key).unwrap_or_else(|e| panic!("key: {e}"));
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
        })
    });
    cx.run_until_parked();
}

/// A hardware letter: GPUI's KeyDown/KeyUp, which the terminal declines, then
/// UIKit typing the key itself (`insertText:` through `replace_text_in_range`).
fn letter(view: &Entity<Smoke>, cx: &mut VisualTestContext, key: &'static str) {
    press(view, cx, key);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // The fixture peer keeps its fixed size, so a render's resize
            // request would hold the text; send it now, as the P8 tests do.
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.replace_text_in_range(None, key, window, cx);
            this.flush_terminal();
        })
    });
    cx.run_until_parked();
}

fn received(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &mpsc::Receiver<ClientMessage>,
) -> Vec<ClientPaneInputEvent> {
    view.update(cx, |this, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.flush_terminal();
        this.terminal
            .link
            .handle
            .as_ref()
            .unwrap_or_else(|| panic!("handle"))
            .send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste(END.into())],
            )
            .unwrap_or_else(|e| panic!("sentinel: {e}"));
    });
    let mut events = Vec::new();
    loop {
        let message = wire
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire: {e}"));
        if let ClientMessage::ClientShellPaneInput {
            pane_id,
            events: batch,
        } = message
        {
            assert_eq!(pane_id, "w1:p1");
            for event in batch {
                if event == ClientPaneInputEvent::Paste(END.into()) {
                    return events;
                }
                events.push(event);
            }
        }
    }
}

fn text(events: &[ClientPaneInputEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| match event {
            ClientPaneInputEvent::TextCommit(text) => text.clone(),
            ClientPaneInputEvent::Key {
                code: ClientKeyCode::Char(c),
                modifiers: 2,
                kind: ClientKeyKind::Press,
                ..
            } => format!("ctrl-{c}"),
            other => format!("{other:?}"),
        })
        .collect()
}

/// Steps 7-10 and 24: tap or click the terminal, then type.
#[gpui::test]
fn sh1_156_tap_after_discard_lets_letters_reach_the_terminal(cx: &mut TestAppContext) {
    for hover in [false, true] {
        let case = if hover { "pointer click" } else { "finger tap" };
        let (view, cx, wire) = connected(cx);
        discard_composition(&view, cx);
        tap(&view, cx, hover);
        view.update(cx, |this, _| {
            assert!(!this.handoff.select_mode, "{case}: tap turned Select on");
        });
        letter(&view, cx, "a");
        press(&view, cx, "ctrl-a");
        letter(&view, cx, "b");
        assert_eq!(
            text(&received(&view, cx, &wire)),
            ["a", "ctrl-a", "b"],
            "{case}: letters after the tap"
        );
        view.update(cx, |this, _| this.terminal.link.pause());
    }
}

/// Steps 12-13 and 24-25: after the tap, the owner pressed the Select control
/// and then turned it off again; letters must reach the terminal after each.
#[gpui::test]
fn sh1_156_select_on_then_off_after_discard_releases_input(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    discard_composition(&view, cx);
    tap(&view, cx, false);
    // The Select control's handler (handoff_view.rs).
    view.update(cx, |this, _| this.toggle_selection());
    assert!(view.update(cx, |this, _| this.handoff.select_mode));
    // A fresh letter ends Select mode and reaches the terminal.
    letter(&view, cx, "c");
    view.update(cx, |this, _| {
        assert!(!this.handoff.select_mode, "a new key left Select on");
        this.toggle_selection();
        assert!(this.handoff.select_mode);
        this.toggle_selection();
        assert!(!this.handoff.select_mode);
        assert!(this.handoff.selection.is_none());
        assert!(
            !this.terminal.input.requires_gesture(),
            "Select off kept the discard fence"
        );
    });
    letter(&view, cx, "e");
    assert_eq!(
        text(&received(&view, cx, &wire)),
        ["c", "e"],
        "Select on then off"
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}

/// One hardware letter in UIKit's order: GPUI's KeyDown (declined), UIKit
/// typing the key (`insertText:`), then the KeyUp that ends the press.
fn typed(view: &Entity<Smoke>, cx: &mut VisualTestContext, key: &'static str) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let keystroke = Keystroke::parse(key).unwrap_or_else(|e| panic!("key: {e}"));
            this.terminal_key_down(
                &KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.replace_text_in_range(None, key, window, cx);
            this.flush_terminal();
            this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
        })
    });
    cx.run_until_parked();
}

fn settings_sheet_round_trip(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.begin_settings_modal(window, cx);
            this.end_settings_modal(window, cx);
            assert!(
                this.terminal_focus.is_focused(window),
                "the sheet closed onto the terminal"
            );
        })
    });
    cx.run_until_parked();
}

/// Review F1: the settings sheet fences late native commits, but that fence
/// holds no discarded composition, so the first letter typed after the sheet
/// closes must reach the terminal. A real discard stays fenced (PR 40).
#[gpui::test]
fn sh1_156_first_letter_after_settings_sheet_reaches_the_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    settings_sheet_round_trip(&view, cx);
    typed(&view, cx, "a");
    typed(&view, cx, "b");
    assert_eq!(
        text(&received(&view, cx, &wire)),
        ["a", "b"],
        "first letters after the settings sheet"
    );
    discard_composition(&view, cx);
    settings_sheet_round_trip(&view, cx);
    typed(&view, cx, "x");
    view.update(cx, |this, _| {
        assert!(
            this.terminal.input.requires_gesture(),
            "a key released a discarded composition"
        );
    });
    assert!(
        received(&view, cx, &wire).is_empty(),
        "discarded commit sent"
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}

/// Review F3, as far as the host reaches: after a discard, letters typed with
/// no tap and no Select stay fenced until the native retirement completes;
/// then letters and a new composition reach the terminal. The host supplies
/// the SDK completion through the ticket code native_retirement.rs drives
/// (begin, prepare, acknowledge); the SDK request itself is iOS-only.
#[gpui::test]
fn sh1_156_letters_after_discard_resume_on_retirement_without_tap(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    discard_composition(&view, cx);
    // UIKit may commit the discarded text with this press; it stays fenced.
    typed(&view, cx, "x");
    let stale = view.update(cx, |this, _| {
        assert!(
            this.terminal.input.requires_gesture(),
            "a key released the fence"
        );
        assert!(!this.handoff.select_mode);
        assert!(this.terminal_status().starts_with("Text input paused"));
        this.begin_text_retirement(40)
            .unwrap_or_else(|| panic!("discarded active owner"))
    });
    // A composition started inside the fence is refused and moves the
    // revision: the ticket issued before it can no longer release the fence.
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_and_mark_text_in_range(None, "か", Some(0..1), window, cx);
            assert!(this.terminal.input.composition.text.is_empty());
        })
    });
    view.update(cx, |this, _| {
        assert!(!this.complete_text_retirement(&stale, 40, 41));
        assert!(
            this.terminal.input.requires_gesture(),
            "stale ticket released"
        );
        this.controlled_text_retirement();
        assert!(!this.terminal.input.requires_gesture());
        assert!(this.retirement.reason.is_none());
    });
    typed(&view, cx, "y");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_and_mark_text_in_range(None, "て", Some(0..1), window, cx);
            assert_eq!(this.terminal.input.composition.text, "て");
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.replace_text_in_range(None, "て", window, cx);
            this.flush_terminal();
        })
    });
    assert_eq!(
        text(&received(&view, cx, &wire)),
        ["y", "て"],
        "letters after the retirement, without a tap"
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}
