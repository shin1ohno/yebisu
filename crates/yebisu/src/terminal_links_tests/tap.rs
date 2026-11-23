//! L66 through GPUI's real mouse dispatch to the terminal element
//! (terminal_view.rs), on the Herdr 1 wire peer showing the link fixture: a
//! press with Command held opens a web address through the platform's open
//! (UIKit's `openURL:options:completionHandler:` on iPadOS, GPUI's test
//! platform here, which records it) and reaches nothing else; a press
//! without Command is today's. These prove app routing, not UIKit delivery.
use crate::{
    app::Smoke,
    mock_peer,
    terminal_links::{Refusal, qa},
};
use gpui::{
    Entity, Modifiers, MouseButton, TestAppContext, TouchEvent, TouchId, TouchPhase,
    VisualTestContext, point, px, size,
};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

const END: &str = "L66-tap-end";

fn command() -> Modifiers {
    Modifiers {
        platform: true,
        ..Modifiers::default()
    }
}

/// A connected Herdr 1 terminal whose frame shows the link fixture.
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
        assert!(Instant::now() < deadline, "L66 frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, _| {
        let Some(shown) = this.terminal.live.surface.as_ref() else {
            panic!("surface");
        };
        let mut surface = (**shown).clone();
        qa::herdr1_fixture(&mut surface.frame);
        this.terminal.live.surface = Some(Arc::new(surface));
    });
    cx.run_until_parked();
    (view, cx, wire)
}

/// The window point at the centre of fixture link `name`'s press cell.
fn target(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    name: &str,
) -> gpui::Point<gpui::Pixels> {
    view.update(cx, |this, _| {
        let Some((_, surface)) = this.terminal.coherent_frame() else {
            panic!("coherent frame");
        };
        let Some((_, (column, row))) = qa::targets(&surface)
            .into_iter()
            .find(|(found, _)| *found == name)
        else {
            panic!("fixture link {name} not found");
        };
        let g = this.terminal.geometry;
        assert!(g.grid.size.height > px(60.), "terminal must be laid out");
        g.grid.origin
            + point(
                px((f32::from(column) + 0.5) * g.cell_width),
                px((f32::from(row) + 0.5) * g.cell_height),
            )
    })
}

/// One press and release at `at`: a pointer click when `modifiers` carry
/// Command, a finger tap (which GPUI dispatches without modifiers) when not.
fn press(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, modifiers: Modifiers) {
    cx.simulate_mouse_move(at, None, modifiers);
    cx.simulate_mouse_down(at, MouseButton::Left, modifiers);
    cx.simulate_mouse_up(at, MouseButton::Left, modifiers);
    cx.run_until_parked();
}

/// Every input event the terminal received before a sentinel paste sent now.
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
        if let ClientMessage::ClientShellPaneInput { events: batch, .. } = message {
            for event in batch {
                if event == ClientPaneInputEvent::Paste(END.into()) {
                    return events;
                }
                events.push(event);
            }
        }
    }
}

/// A Command click on fixture link `name` opens `url` and nothing else.
fn command_click_opens(cx: &mut TestAppContext, name: &str, url: &str) {
    let (view, cx, wire) = connected(cx);
    let at = target(&view, cx, name);
    press(cx, at, command());
    assert_eq!(cx.opened_url().as_deref(), Some(url), "{name}");
    view.update(cx, |this, _| {
        assert_eq!(
            this.terminal_status(),
            "Opening example.com in the browser",
            "{name}"
        );
        assert!(this.handoff.selection.is_none(), "{name}: no selection");
        assert!(!this.handoff.passive_highlight, "{name}: no tap highlight");
    });
    assert_eq!(
        received(&view, cx, &wire),
        [],
        "{name}: Command + click sent input"
    );
}

/// After wide characters, and without the full stop after it.
#[gpui::test]
fn command_click_opens_a_plain_address_as_shown(cx: &mut TestAppContext) {
    command_click_opens(cx, "plain", qa::PLAIN);
}

#[gpui::test]
fn command_click_opens_a_wrapped_address_whole(cx: &mut TestAppContext) {
    command_click_opens(cx, "wrapped", qa::WRAPPED);
}

#[gpui::test]
fn command_click_opens_an_osc8_hyperlink_target(cx: &mut TestAppContext) {
    command_click_opens(cx, "osc8", qa::OSC8);
}

#[gpui::test]
fn a_finger_tap_reads_command_held_on_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    let at = target(&view, cx, "plain");
    // A finger's touches become a tap in GPUI's gesture recognizer, which
    // dispatches it without modifiers; the hardware keyboard's Command
    // arrives as ModifiersChanged (herdr-uikit text_input/keys.rs).
    let finger = |cx: &mut VisualTestContext| {
        for phase in [TouchPhase::Started, TouchPhase::Ended] {
            cx.simulate_event(TouchEvent {
                id: TouchId(66),
                phase,
                position: at,
                predicted_position: None,
                force: None,
            });
        }
        cx.run_until_parked();
    };
    finger(cx);
    assert_eq!(cx.opened_url(), None, "a tap without Command opened");
    cx.simulate_modifiers_change(command());
    finger(cx);
    assert_eq!(cx.opened_url().as_deref(), Some(qa::PLAIN));
    cx.simulate_modifiers_change(Modifiers::default());
    assert_eq!(received(&view, cx, &wire), [], "Command + tap sent input");
}

#[gpui::test]
fn without_command_a_press_on_a_link_is_todays_press(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    for name in ["plain", "osc8", "mailto"] {
        let at = target(&view, cx, name);
        press(cx, at, Modifiers::default());
        assert_eq!(cx.opened_url(), None, "{name}: opened without Command");
        view.update(cx, |this, _| {
            // Start: Drag's tap: a one-cell passive highlight, and input asked
            // for again (SH1-156), exactly as on any other cell.
            assert!(this.handoff.passive_highlight, "{name}: not today's tap");
            assert!(
                !this.terminal_status().starts_with("Not opened"),
                "{name}: a refusal without Command"
            );
        });
    }
}

#[gpui::test]
fn other_schemes_and_paths_open_nothing_and_say_why(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    for (name, refusal) in [
        ("mailto", Refusal::Scheme("mailto".into())),
        ("path", Refusal::Path),
    ] {
        let at = target(&view, cx, name);
        press(cx, at, command());
        assert_eq!(cx.opened_url(), None, "{name}");
        view.update(cx, |this, _| {
            assert_eq!(this.terminal_status(), refusal.reason(), "{name}");
            assert!(this.handoff.selection.is_none(), "{name}: no selection");
        });
    }
    // Command over text that is no link: today's press.
    let plain = target(&view, cx, "plain");
    let word = plain
        - point(
            px(0.),
            px(view.update(cx, |this, _| this.terminal.geometry.cell_height)),
        );
    press(cx, word, command());
    assert_eq!(cx.opened_url(), None);
    assert_eq!(received(&view, cx, &wire), [], "a refused press sent input");
}
