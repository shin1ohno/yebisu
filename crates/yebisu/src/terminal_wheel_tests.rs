//! Issue #137 PR 1 through GPUI's real scroll dispatch to the terminal's
//! wheel canvas, on the Herdr 1 wire peer. Each guard has its own test.
use super::*;
use crate::{app::Smoke, mock_peer};
use gpui::{
    Bounds, Entity, Modifiers, Pixels, Point, Render, ScrollDelta, ScrollSource, TestAppContext,
    TouchEvent, TouchId, TouchPhase, VisualTestContext, div, point, px, size,
};
use sheprd_client::protocol::{
    ClientMessage, PaneSurfacePane, PaneSurfaceScrollMetrics, SurfaceRect,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[path = "terminal_wheel_tests/coast.rs"]
mod coast;
#[path = "terminal_wheel_tests/herdr2.rs"]
mod herdr2;
#[path = "terminal_wheel_tests/jump.rs"]
mod jump;
#[path = "terminal_wheel_tests/sheets.rs"]
mod sheets;

const END: &str = "issue-137-end";
const FOCUSED: &str = "w1:p1";
const OTHER: &str = "w1:p9";

pub(super) type Wire = mpsc::Receiver<ClientMessage>;

/// A connected Herdr 1 terminal on the quiet peer, drawn at 1200x1000.
fn connected(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext, Wire) {
    connected_with(cx, mock_peer::connect_quiet)
}

fn connected_with(
    cx: &mut TestAppContext,
    peer: fn() -> crate::Result<(sheprd_client::Client, Wire)>,
) -> (Entity<Smoke>, &mut VisualTestContext, Wire) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, wire) = peer().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("issue-137");
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
        assert!(Instant::now() < deadline, "peer frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    (view, cx, wire)
}

fn pane(id: &str, x: u16, width: u16, height: u16) -> PaneSurfacePane {
    let rect = SurfaceRect {
        x,
        y: 0,
        width,
        height,
    };
    PaneSurfacePane {
        pane_id: id.into(),
        content_revision: 1,
        rect,
        inner_rect: rect,
        scrollbar_rect: None,
        scroll: None,
        focused: id == FOCUSED,
        mouse_reporting: false,
        sgr_pixel_mouse: false,
        alternate_screen_active: false,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// One pane over the whole frame, edited by `edit`.
fn single(edit: impl FnOnce(&mut PaneSurfacePane)) -> Vec<PaneSurfacePane> {
    let mut only = pane(FOCUSED, 0, 80, 24);
    edit(&mut only);
    vec![only]
}

/// Shows `panes` in the drawn copy and in the reader's mailbox, which the
/// send path checks again.
fn install(view: &Entity<Smoke>, cx: &mut VisualTestContext, panes: Vec<PaneSurfacePane>) {
    view.update(cx, |this, _| {
        let Some(shown) = this.terminal.live.surface.as_ref() else {
            panic!("surface");
        };
        let mut surface = (**shown).clone();
        surface.panes = panes;
        let surface = Arc::new(surface);
        this.terminal.live.surface = Some(surface.clone());
        match this.terminal.link.inbox.lock() {
            Ok(mut live) => live.surface = Some(surface),
            Err(_) => panic!("inbox"),
        }
    });
    cx.run_until_parked();
}

/// Input stays held while the peer's frame differs from the requested
/// size; the quiet peer never answers a resize. Clear it, as a matching
/// frame would.
fn ready(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
    });
}

/// The window point at the centre of `pane`'s drawn body (a split), or of
/// cell (10, 5) of the single grid.
fn over(view: &Entity<Smoke>, cx: &mut VisualTestContext, pane: &str) -> Point<Pixels> {
    view.update(cx, |this, _| {
        let Some((_, surface)) = this.terminal.coherent_frame() else {
            panic!("coherent frame");
        };
        let g = this.terminal.geometry;
        if let Some(placement) = g
            .framed_panes(&surface)
            .into_iter()
            .find(|placement| placement.pane.pane_id == pane)
        {
            return placement.body.center();
        }
        g.grid.origin + point(px(10.5 * g.cell_width), px(5.5 * g.cell_height))
    })
}

fn cell_height(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> f32 {
    view.read_with(cx, |this, _| this.terminal.geometry.cell_height)
}

fn scroll(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    at: Point<Pixels>,
    delta: ScrollDelta,
    phase: TouchPhase,
) {
    ready(view, cx);
    cx.simulate_event(ScrollEvent {
        position: at,
        delta,
        modifiers: Modifiers::none(),
        touch_phase: phase,
        source: ScrollSource::Touch,
    });
}

fn pixels(x: f32, y: f32) -> ScrollDelta {
    ScrollDelta::Pixels(point(px(x), px(y)))
}

/// Every message the peer received before a sentinel paste sent now.
fn messages(view: &Entity<Smoke>, cx: &mut VisualTestContext, wire: &Wire) -> Vec<ClientMessage> {
    view.update(cx, |this, _| {
        this.terminal
            .link
            .handle
            .as_ref()
            .unwrap_or_else(|| panic!("handle"))
            .send_input(
                "boot-v1",
                FOCUSED,
                [ClientPaneInputEvent::Paste(END.into())],
            )
            .unwrap_or_else(|e| panic!("sentinel: {e}"));
    });
    let mut seen = Vec::new();
    loop {
        let message = wire
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire: {e}"));
        if let ClientMessage::ClientShellPaneInput { events, .. } = &message
            && events.contains(&ClientPaneInputEvent::Paste(END.into()))
        {
            return seen;
        }
        seen.push(message);
    }
}

/// The wheel events each pane received, in order.
fn wheels(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &Wire,
) -> Vec<(String, ClientMouseKind, u16)> {
    messages(view, cx, wire)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::ClientShellPaneInput { pane_id, events } => Some((pane_id, events)),
            _ => None,
        })
        .flat_map(|(pane, events)| {
            events.into_iter().filter_map(move |event| match event {
                ClientPaneInputEvent::Mouse { kind, lines, .. } => {
                    Some((pane.clone(), kind, lines))
                }
                _ => None,
            })
        })
        .collect()
}

fn up(pane: &str, lines: u16) -> (String, ClientMouseKind, u16) {
    (pane.into(), ClientMouseKind::ScrollUp, lines)
}

/// T1: a pan of 3.5 rows is three lines up, keeping the half; the next
/// half row completes the fourth.
#[gpui::test]
fn a_pan_scrolls_the_scrollback_in_whole_lines_and_keeps_the_rest(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let at = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    scroll(&view, cx, at, pixels(0., 3.5 * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), [up(FOCUSED, 3)]);
    scroll(&view, cx, at, pixels(0., 0.5 * row), TouchPhase::Moved);
    assert_eq!(wheels(&view, cx, &wire), [up(FOCUSED, 1)]);
    scroll(&view, cx, at, pixels(0., -2. * row), TouchPhase::Moved);
    assert_eq!(
        wheels(&view, cx, &wire),
        [(FOCUSED.into(), ClientMouseKind::ScrollDown, 2)]
    );
}

/// T2: a program reading the mouse gets one report per line and the
/// horizontal motion; the alternate screen gets one arrow per line only.
#[gpui::test]
fn a_program_reading_the_wheel_gets_one_event_per_line(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    let at = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    let width = view.read_with(cx, |this, _| this.terminal.geometry.cell_width);
    install(&view, cx, single(|p| p.mouse_reporting = true));
    scroll(&view, cx, at, pixels(0., 3. * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), vec![up(FOCUSED, 1); 3]);
    scroll(&view, cx, at, pixels(2. * width, 0.), TouchPhase::Started);
    assert_eq!(
        wheels(&view, cx, &wire),
        vec![(FOCUSED.to_owned(), ClientMouseKind::ScrollLeft, 1); 2]
    );
    install(&view, cx, single(|p| p.alternate_screen_active = true));
    scroll(&view, cx, at, pixels(0., 3. * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), vec![up(FOCUSED, 1); 3]);
    scroll(&view, cx, at, pixels(2. * width, 0.), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), []);
}

/// T2b: a wheel notch scrolls Herdr's default three lines.
#[gpui::test]
fn a_wheel_notch_scrolls_three_lines(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let at = over(&view, cx, FOCUSED);
    let notch = f32::from(crate::upstream::wheel::PLATFORM_NOTCH_LINES);
    scroll(
        &view,
        cx,
        at,
        ScrollDelta::Lines(point(0., notch)),
        TouchPhase::Moved,
    );
    assert_eq!(wheels(&view, cx, &wire), [up(FOCUSED, 3)]);
}

/// T3, T4: a finger's pan over the terminal scrolls it and selects
/// nothing; a finger landing again stops the motion without a tap and
/// without another wheel event.
#[gpui::test]
fn a_finger_pan_scrolls_and_neither_selects_nor_taps(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let start = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    let touch = |cx: &mut VisualTestContext, phase, position| {
        cx.simulate_event(TouchEvent {
            id: TouchId(137),
            phase,
            position,
            predicted_position: None,
            force: None,
        });
    };
    ready(&view, cx);
    touch(cx, TouchPhase::Started, start);
    for step in 1..=6 {
        std::thread::sleep(Duration::from_millis(8));
        ready(&view, cx);
        touch(
            cx,
            TouchPhase::Moved,
            start + point(px(0.), px(step as f32 * row)),
        );
    }
    ready(&view, cx);
    touch(cx, TouchPhase::Ended, start + point(px(0.), px(6. * row)));
    cx.run_until_parked();
    let sent = wheels(&view, cx, &wire);
    let lines: u16 = sent.iter().map(|(_, _, lines)| lines).sum();
    assert!(
        sent.iter()
            .all(|(pane, kind, _)| pane == FOCUSED && *kind == ClientMouseKind::ScrollUp),
        "{sent:?}"
    );
    assert!(lines >= 4, "a six-row pan scrolled {lines} lines: {sent:?}");
    view.read_with(cx, |this, _| {
        assert!(this.handoff.selection.is_none(), "a pan selected text");
    });
    // A finger landing again stops any motion and is no tap: the tap
    // would focus the terminal, which a pan never does.
    let taps = view.read_with(cx, |this, _| this.taps);
    ready(&view, cx);
    touch(cx, TouchPhase::Started, start);
    std::thread::sleep(Duration::from_millis(30));
    ready(&view, cx);
    touch(cx, TouchPhase::Cancelled, start);
    cx.run_until_parked();
    assert_eq!(wheels(&view, cx, &wire), [], "the landing finger scrolled");
    assert_eq!(view.read_with(cx, |this, _| this.taps), taps);
    view.read_with(cx, |this, _| assert!(this.handoff.selection.is_none()));
}

/// T5: in a split, the pane under the finger gets the wheel, focused or not.
#[gpui::test]
fn the_pane_under_the_finger_gets_the_wheel_in_a_split(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(
        &view,
        cx,
        vec![pane(FOCUSED, 0, 40, 24), pane(OTHER, 41, 39, 24)],
    );
    let row = cell_height(&view, cx);
    for target in [OTHER, FOCUSED] {
        let at = over(&view, cx, target);
        scroll(&view, cx, at, pixels(0., 2. * row), TouchPhase::Started);
        assert_eq!(wheels(&view, cx, &wire), [up(target, 2)], "{target}");
    }
    view.read_with(cx, |this, _| {
        assert!(
            !this.terminal.live.status.contains("discarded"),
            "{}",
            this.terminal.live.status
        );
    });
}

/// The send path checks the reader's newest surface again: a pane drawn
/// but no longer published gets nothing.
#[gpui::test]
fn a_pane_the_reader_no_longer_lists_gets_nothing(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(
        &view,
        cx,
        vec![pane(FOCUSED, 0, 40, 24), pane(OTHER, 41, 39, 24)],
    );
    match view
        .read_with(cx, |this, _| this.terminal.link.inbox.clone())
        .lock()
    {
        Ok(mut live) => {
            let Some(shown) = live.surface.as_ref() else {
                panic!("surface");
            };
            let mut surface = (**shown).clone();
            surface.panes.retain(|p| p.pane_id == FOCUSED);
            live.surface = Some(Arc::new(surface));
        }
        Err(_) => panic!("inbox"),
    }
    let row = cell_height(&view, cx);
    let at = over(&view, cx, OTHER);
    scroll(&view, cx, at, pixels(0., 2. * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), []);
    view.read_with(cx, |this, _| {
        assert!(
            this.terminal.live.status.contains("Input target changed"),
            "{}",
            this.terminal.live.status
        );
    });
}

/// T6: an occluding sheet or popover in front of the terminal (•••,
/// Shortcuts, Settings, Hosts) takes the scroll; the terminal gets nothing.
#[gpui::test]
fn an_occluding_sheet_or_popover_keeps_the_scroll(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let row = cell_height(&view, cx);
    type Open = fn(&mut Smoke);
    let cases: [(&str, Open, Open); 4] = [
        (
            "more menu",
            |this| this.shell.frame.overflow_open = true,
            |this| this.shell.frame.overflow_open = false,
        ),
        (
            "shortcuts",
            |this| this.shell.shortcuts.open = true,
            |this| this.shell.shortcuts.open = false,
        ),
        (
            "settings",
            |this| this.settings_modal = true,
            |this| this.settings_modal = false,
        ),
        // As the footer's Hosts button opens it over a terminal (`open_hosts`).
        (
            "hosts",
            |this| {
                this.shell.hosts_sheet.open_list();
                this.shell.hosts_sheet.over_terminal = true;
            },
            |this| this.shell.hosts_sheet.close(),
        ),
    ];
    for (name, open, close) in cases {
        view.update(cx, |this, cx| {
            open(this);
            cx.notify();
        });
        cx.run_until_parked();
        if name == "hosts" {
            assert!(
                cx.debug_bounds("hosts-overlay").is_some(),
                "the Hosts sheet is not over the terminal"
            );
        }
        // The more menu floats over the grid's top right corner.
        let at = if name == "more menu" {
            let menu = cx
                .debug_bounds("shell-overflow-menu")
                .unwrap_or_else(|| panic!("menu not painted"));
            let grid = view.read_with(cx, |this, _| this.terminal.geometry.grid);
            point(menu.center().x, grid.top() + px(10.))
        } else {
            over(&view, cx, FOCUSED)
        };
        scroll(&view, cx, at, pixels(0., 3. * row), TouchPhase::Started);
        assert_eq!(wheels(&view, cx, &wire), [], "{name}");
        view.update(cx, |this, cx| {
            close(this);
            cx.notify();
        });
        cx.run_until_parked();
    }
}

struct Overlay {
    view: Entity<Smoke>,
    layer: Bounds<Pixels>,
    parent: Rc<RefCell<usize>>,
}
impl Render for Overlay {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = self.view.read(cx).front_layers.clone();
        let parent = self.parent.clone();
        div()
            .relative()
            .size_full()
            .on_scroll(move |_, _, _| *parent.borrow_mut() += 1)
            .child(self.view.clone())
            .child(
                div()
                    .absolute()
                    .left(self.layer.origin.x)
                    .top(self.layer.origin.y)
                    .w(self.layer.size.width)
                    .h(self.layer.size.height)
                    .child(crate::front_layer::front_layer(
                        div()
                            .w(self.layer.size.width)
                            .h(self.layer.size.height)
                            .into_any_element(),
                        registry,
                    )),
            )
    }
}

/// The window, wrapped in a parent that counts scrolls reaching it, with a
/// front layer at `layer`.
fn wrapped(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    layer: Bounds<Pixels>,
) -> Rc<RefCell<usize>> {
    let parent = Rc::new(RefCell::new(0));
    cx.update(|window, cx| {
        let parent = parent.clone();
        let view = view.clone();
        window.replace_root(cx, move |_, _| Overlay {
            view,
            layer,
            parent,
        });
    });
    cx.run_until_parked();
    parent
}

/// T7, T9: a front layer over the pointer keeps the scroll from the
/// terminal; elsewhere the terminal takes it and stops it there.
#[gpui::test]
fn a_front_layer_keeps_the_scroll_and_the_terminal_stops_its_own(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let at = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    let layer = Bounds::new(at - point(px(40.), px(20.)), size(px(80.), px(40.)));
    let parent = wrapped(&view, cx, layer);
    scroll(&view, cx, at, pixels(0., 2. * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), [], "under the front layer");
    assert_eq!(*parent.borrow(), 1, "the scroll went on to the parent");
    let beside = at + point(px(0.), px(6. * row));
    scroll(&view, cx, beside, pixels(0., 2. * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), [up(FOCUSED, 2)]);
    assert_eq!(*parent.borrow(), 1, "the terminal did not stop its scroll");
}

/// T8: search, a composition, the size hold, a held selection and input
/// that is not ready each keep the wheel from the terminal.
#[gpui::test]
fn find_composition_and_unready_input_keep_the_wheel(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let at = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    type Set = fn(&mut Smoke, bool);
    let cases: [(&str, Set); 4] = [
        ("find", |this, on| this.find_handoff.paused = on),
        ("composition", |this, on| {
            this.terminal.input.composition.marked = on.then_some(0..1);
        }),
        ("size hold", |this, on| {
            this.terminal.input.hold_for_size(on)
        }),
        ("selection", |this, on| this.handoff.select_mode = on),
    ];
    for (name, set) in cases {
        view.update(cx, |this, _| set(this, true));
        scroll(&view, cx, at, pixels(0., 2. * row), TouchPhase::Started);
        view.update(cx, |this, _| set(this, false));
        assert_eq!(wheels(&view, cx, &wire), [], "{name}");
    }
    // Not ready: the requested size has no matching frame yet.
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(view.read_with(cx, |this, _| this.terminal.size.is_some()));
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: pixels(0., 2. * row),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Started,
        source: ScrollSource::Touch,
    });
    assert_eq!(wheels(&view, cx, &wire), [], "not ready");
    scroll(&view, cx, at, pixels(0., 2. * row), TouchPhase::Started);
    assert_eq!(wheels(&view, cx, &wire), [up(FOCUSED, 2)], "ready again");
}

fn scrolled(offset: u64, edit: impl FnOnce(&mut PaneSurfacePane)) -> Vec<PaneSurfacePane> {
    single(|pane| {
        pane.scroll = Some(PaneSurfaceScrollMetrics {
            offset_from_bottom: offset,
            max_offset_from_bottom: 500,
            viewport_rows: 24,
        });
        edit(pane);
    })
}

/// Starts or continues a composition through Smoke's text input callback.
fn mark(view: &Entity<Smoke>, cx: &mut VisualTestContext, text: &str) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            gpui::EntityInputHandler::replace_and_mark_text_in_range(
                this, None, text, None, window, cx,
            );
        });
    });
    cx.run_until_parked();
}

fn pane_scrolls(messages: &[ClientMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter_map(|message| match message {
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                serde_json::from_str::<serde_json::Value>(request).ok()
            }
            _ => None,
        })
        .filter(|request| request["method"] == "pane.scroll")
        .map(|request| request["params"].clone())
        .collect()
}

/// T11: starting a composition in the scrollback asks Herdr once for the
/// bottom; continuing it, or starting at the bottom, asks nothing.
#[gpui::test]
fn starting_a_composition_returns_the_scrollback_to_the_bottom(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected_with(cx, mock_peer::connect_quiet_scroll);
    assert!(view.read_with(cx, |this, _| {
        Method::PaneScroll.advertised_in(&this.terminal.live.methods)
    }));
    install(&view, cx, scrolled(120, |_| {}));
    mark(&view, cx, "k");
    mark(&view, cx, "か");
    let sent = messages(&view, cx, &wire);
    assert_eq!(
        pane_scrolls(&sent),
        [serde_json::json!({"pane_id": FOCUSED, "offset_from_bottom": 0})]
    );
    view.update(cx, |this, _| {
        this.terminal.input.composition = Default::default();
    });
    install(&view, cx, scrolled(0, |_| {}));
    mark(&view, cx, "k");
    assert_eq!(
        pane_scrolls(&messages(&view, cx, &wire)),
        Vec::<serde_json::Value>::new()
    );
}

/// T12: without `pane.scroll`, the wheel returns the scrollback, but never
/// to a program that would read it as reports or arrow keys.
#[gpui::test]
fn without_pane_scroll_the_wheel_returns_only_the_scrollback(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    type Edit = fn(&mut PaneSurfacePane);
    let cases: [(&str, Edit, bool); 3] = [
        ("scrollback", |_| {}, true),
        ("mouse reporting", |p| p.mouse_reporting = true, false),
        (
            "alternate screen",
            |p| p.alternate_screen_active = true,
            false,
        ),
    ];
    for (name, edit, sends) in cases {
        view.update(cx, |this, _| {
            this.terminal.input.composition = Default::default();
        });
        install(&view, cx, scrolled(70, edit));
        mark(&view, cx, "k");
        let expected = if sends {
            vec![(FOCUSED.to_owned(), ClientMouseKind::ScrollDown, 70)]
        } else {
            vec![]
        };
        assert_eq!(wheels(&view, cx, &wire), expected, "{name}");
    }
}
