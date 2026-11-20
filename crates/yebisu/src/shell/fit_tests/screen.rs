//! L9 rules on the real screen: each test draws `Smoke` in a test window, so
//! it goes through the production caller (the frame, the status bar, the
//! terminal screen), not only the pure function (layout follow-up W1/W2).
use crate::{app::Smoke, layout::Regions, mock_peer};
use gpui::{
    Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualContext as _, VisualTestContext,
    point, px, size,
};
use std::time::{Duration, Instant};

/// A Herdr 1 terminal on the private peer, drawn in the frame at `w`×`h`.
fn terminal(cx: &mut TestAppContext, w: f32, h: f32) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(w), px(h)));
    let (client, _receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("fit-screen");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx)
}

fn redraw(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

fn painted(cx: &mut VisualTestContext, selector: &'static str) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(selector)
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    painted(cx, selector).unwrap_or_else(|| panic!("{selector} not painted"))
}

/// S1: `render_too_small` (the terminal screen's caller) holds the input, so
/// a key typed into a too-small window is not queued, and the hold ends when
/// the window grows again.
#[gpui::test]
fn too_small_screen_holds_terminal_input(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 1200., 1000.);
    let typed = |view: &Entity<Smoke>, cx: &mut VisualTestContext| {
        view.update(cx, |this, _| {
            this.terminal
                .input
                .commit("a")
                .is_some_and(|input| this.terminal.input.queue(input))
        })
    };
    assert!(typed(&view, cx), "fixture: a full window queues a key");
    cx.simulate_resize(size(px(320.), px(300.)));
    redraw(&view, cx);
    assert!(painted(cx, "shell-too-small").is_some());
    assert!(
        !typed(&view, cx),
        "a key typed into a too-small window was queued"
    );
    cx.simulate_resize(size(px(1200.), px(1000.)));
    redraw(&view, cx);
    assert!(painted(cx, "shell-too-small").is_none());
    assert!(typed(&view, cx), "the hold outlived the small window");
}

/// A floating keyboard (here: the platform's visible viewport shrinking
/// without a docked keyboard inset) does not change the terminal's size.
#[gpui::test]
fn floating_keyboard_keeps_terminal_size_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 1200., 1000.);
    let before = bounds(cx, "shell-terminal");
    let window = cx.window_handle();
    cx.simulate_window_visual_viewport_change(
        window,
        Bounds::new(point(px(0.), px(0.)), size(px(1200.), px(640.))),
    );
    redraw(&view, cx);
    assert_eq!(
        bounds(cx, "shell-terminal"),
        before,
        "a floating keyboard resized the terminal"
    );
}

/// The status bar moves Hardware keyboard to the details first as it
/// narrows (`fit::status_bar_overflow_with` through `render_status_bar`).
#[gpui::test]
fn status_bar_moves_hardware_keyboard_first_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 1200., 1000.);
    assert!(
        painted(cx, "shell-status-hardware-keyboard").is_none(),
        "no hardware key yet"
    );
    cx.simulate_keystrokes("a");
    redraw(&view, cx);
    assert!(
        painted(cx, "shell-status-hardware-keyboard").is_some(),
        "a hardware key press shows the item in a wide bar"
    );
    cx.simulate_resize(size(px(440.), px(1000.)));
    redraw(&view, cx);
    assert!(
        painted(cx, "shell-status-shortcuts").is_some(),
        "Shortcuts stays in a narrow bar"
    );
    assert!(
        painted(cx, "shell-status-hardware-keyboard").is_none(),
        "a narrow bar kept Hardware keyboard"
    );
}

/// Mods n shows while L0 withdraws a requested region, and tapping it opens
/// the list in overlay O.
#[gpui::test]
fn status_bar_mods_opens_list_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 605., 834.);
    assert!(
        painted(cx, "shell-status-mods").is_none(),
        "no mod requested"
    );
    view.update(cx, |this, _| {
        this.shell.fit.mod_regions = Regions::defaults()
    });
    redraw(&view, cx);
    let mods = bounds(cx, "shell-status-mods");
    assert!(painted(cx, "shell-mods-overlay").is_none());
    cx.simulate_click(
        point(
            mods.origin.x + mods.size.width / 2.,
            mods.origin.y + mods.size.height / 2.,
        ),
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert!(
        painted(cx, "shell-mods-overlay").is_some(),
        "tapping Mods did not open the list"
    );
}

/// The line drops: the mod regions keep their size for 30 s, then go to 0,
/// so the withdrawn count (and Mods) goes too.
#[gpui::test]
fn disconnected_mod_regions_clear_after_30s_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 605., 834.);
    let dropped = |view: &Entity<Smoke>, cx: &mut VisualTestContext, ago: u64| {
        view.update(cx, |this, _| {
            this.shell.fit.mod_regions = Regions::defaults();
            this.terminal.live.connected = false;
            this.shell.fit.disconnect = Default::default();
            let since = Instant::now()
                .checked_sub(Duration::from_secs(ago))
                .unwrap_or_else(|| panic!("clock before {ago} s"));
            this.shell.fit.disconnect.observe(false, since);
        });
        redraw(view, cx);
        view.read_with(cx, |this, _| this.shell.frame.withdrawn)
    };
    assert_eq!(dropped(&view, cx, 29), 1, "the regions left before 30 s");
    assert!(painted(cx, "shell-status-mods").is_some());
    assert_eq!(dropped(&view, cx, 31), 0, "the regions stayed after 30 s");
    assert!(painted(cx, "shell-status-mods").is_none());
}

/// Review F2: the status bar hands the line state to the clock and the 30 s
/// timer draws again, so the regions go to 0 with no input and no output.
#[gpui::test]
fn line_drop_clears_mod_regions_after_30s_without_input(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 605., 834.);
    view.update(cx, |this, _| {
        this.shell.fit.mod_regions = Regions::defaults()
    });
    redraw(&view, cx);
    assert_eq!(view.read_with(cx, |this, _| this.shell.frame.withdrawn), 1);
    // The line drops (the link's inbox too, so the poll keeps it down); the
    // next frame starts the clock.
    view.update(cx, |this, _| {
        this.terminal.live.connected = false;
        if let Ok(mut live) = this.terminal.link.inbox.lock() {
            live.connected = false;
        }
    });
    redraw(&view, cx);
    assert_eq!(view.read_with(cx, |this, _| this.shell.frame.withdrawn), 1);
    // 31 s pass with nothing to draw: only the timer can draw again.
    view.update(cx, |this, _| {
        this.shell.fit.clock_ahead = Duration::from_secs(31);
    });
    cx.executor().advance_clock(Duration::from_secs(31));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |this, _| this.shell.frame.withdrawn),
        0,
        "the regions stayed 31 s after the line dropped"
    );
}

/// Review F3: a keyboard docked along the bottom edge shortens the terminal
/// through `layout_for` (the body of `frame_layout`); a keyboard inset from
/// a side edge (floating) does not. GPUI's test platform cannot change a
/// window's insets from outside the crate, so the insets go in here.
#[gpui::test]
fn docked_keyboard_shortens_terminal_through_frame_layout(cx: &mut TestAppContext) {
    use gpui::{Edges, WindowInsets};
    let (view, cx) = terminal(cx, 1376., 1032.);
    let window = crate::layout::Size::new(1376., 1032.);
    let with = |ime: Edges<Pixels>| WindowInsets {
        ime,
        ..WindowInsets::default()
    };
    let terminal_height = |view: &Entity<Smoke>, cx: &mut VisualTestContext, ime| {
        view.read_with(cx, |this, _| {
            this.layout_for(window, &with(ime), None)
                .terminal
                .rect()
                .map(|rect| rect.height)
                .unwrap_or_else(|| panic!("terminal fits"))
        })
    };
    let bare = terminal_height(&view, cx, Edges::default());
    let docked = terminal_height(
        &view,
        cx,
        Edges {
            bottom: px(320.),
            ..Edges::default()
        },
    );
    assert!(
        (bare - docked - 320.).abs() < 1.,
        "a docked 320 pt keyboard left the terminal at {docked} (bare {bare})"
    );
    let floating = terminal_height(
        &view,
        cx,
        Edges {
            bottom: px(320.),
            left: px(500.),
            right: px(400.),
            ..Edges::default()
        },
    );
    assert_eq!(floating, bare, "a floating keyboard resized the terminal");
}

/// Review F2: the timer `observe_disconnect` starts draws again 30 s after
/// the line drops, in a window where nothing else draws.
#[gpui::test]
fn line_drop_timer_draws_again_after_30s(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| this.smoke_mode = false);
    cx.run_until_parked();
    let draws = Rc::new(Cell::new(0_usize));
    let counter = draws.clone();
    let _observer =
        cx.update(|_, app| app.observe(&view, move |_, _| counter.set(counter.get() + 1)));
    view.update(cx, |this, cx| this.observe_disconnect(false, cx));
    cx.run_until_parked();
    draws.set(0);
    cx.executor().advance_clock(Duration::from_secs(29));
    cx.run_until_parked();
    assert_eq!(draws.get(), 0, "fixture: something else draws this window");
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(draws.get() > 0, "nothing drew 30 s after the line dropped");
}

fn docked(height: f32) -> gpui::WindowInsets {
    gpui::WindowInsets {
        ime: gpui::Edges {
            bottom: px(height),
            ..gpui::Edges::default()
        },
        ..gpui::WindowInsets::default()
    }
}

/// Review G3: `frame_layout` passes the window's own insets on
/// (`platform_insets`), so a docked keyboard shortens the terminal on screen
/// and a floating one does not.
#[gpui::test]
fn docked_keyboard_shortens_terminal_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 1376., 1032.);
    let bare = bounds(cx, "shell-terminal");
    view.update(cx, |this, _| this.shell.fit.insets = Some(docked(320.)));
    redraw(&view, cx);
    let shortened = bounds(cx, "shell-terminal");
    assert!(
        (f32::from(bare.size.height - shortened.size.height) - 320.).abs() < 1.,
        "a docked 320 pt keyboard left the terminal {shortened:?} (bare {bare:?})"
    );
    let mut floating = docked(320.);
    floating.ime.left = px(500.);
    floating.ime.right = px(400.);
    view.update(cx, |this, _| this.shell.fit.insets = Some(floating));
    redraw(&view, cx);
    assert_eq!(
        bounds(cx, "shell-terminal"),
        bare,
        "a floating keyboard resized the terminal"
    );
}

/// Review G2 (the F5 fix): the docked software keyboard clears Hardware
/// keyboard, and its own presses (Return, Backspace) do not bring it back
/// after it closes; a printable hardware press does.
#[gpui::test]
fn software_keyboard_clears_hardware_keyboard_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx, 1200., 1000.);
    let item = |cx: &mut VisualTestContext| painted(cx, "shell-status-hardware-keyboard").is_some();
    cx.simulate_keystrokes("a");
    redraw(&view, cx);
    assert!(item(cx), "fixture: a hardware press shows the item");
    view.update(cx, |this, _| this.shell.fit.insets = Some(docked(320.)));
    redraw(&view, cx);
    assert!(
        !item(cx),
        "the docked software keyboard kept Hardware keyboard"
    );
    view.update(cx, |this, _| this.shell.fit.insets = None);
    cx.simulate_keystrokes("enter backspace");
    redraw(&view, cx);
    assert!(
        !item(cx),
        "the software keyboard's Return or Backspace showed the item"
    );
    cx.simulate_keystrokes("b");
    redraw(&view, cx);
    assert!(
        item(cx),
        "a hardware press after the software keyboard was not counted"
    );
}
