use super::*;
use crate::board::snapshot;
use gpui::{ScrollDelta, ScrollEvent, ScrollSource, TouchPhase, WindowInsets};

fn tap(cx: &mut VisualTestContext, selector: &'static str) {
    let at = bounds(cx, selector).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn status_detail_mark_and_panel_disappear_with_the_message(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let set_status = |cx: &mut VisualTestContext, status: &str| {
        view.update(cx, |this, _| {
            this.terminal.input.fresh_gesture();
            this.terminal.live.status = status.into();
        });
        redraw(&view, cx);
    };
    set_status(cx, "Connected");
    assert!(cx.debug_bounds("shell-status-detail-mark").is_none());
    set_status(cx, "Paste queued");
    assert!(cx.debug_bounds("shell-status-detail-mark").is_some());
    assert!(cx.debug_bounds("shell-status-detail").is_none());
    tap(cx, "shell-status-line");
    assert!(cx.debug_bounds("shell-status-detail").is_some());

    set_status(cx, "Connected");
    assert!(cx.debug_bounds("shell-status-detail").is_none());
    assert!(cx.debug_bounds("shell-status-detail-mark").is_none());

    set_status(cx, "Second connection detail");
    assert!(cx.debug_bounds("shell-status-detail-mark").is_some());
    assert!(cx.debug_bounds("shell-status-detail").is_none());
}

#[gpui::test]
fn status_detail_dismisses_without_taking_the_terminal_press(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| {
        this.terminal.live.status = "Paste queued".into();
        this.handoff.select_mode = true;
    });
    redraw(&view, cx);
    assert!(cx.debug_bounds("shell-selection-status").is_some());
    tap(cx, "shell-status-line");
    let panel = bounds(cx, "shell-status-detail");
    let grid = view.read_with(cx, |this, _| this.terminal.geometry.grid);
    let at = point(
        (panel.left().max(grid.left()) + panel.right().min(grid.right())) / 2.,
        (panel.top().max(grid.top()) + panel.bottom().min(grid.bottom())) / 2.,
    );
    assert!(
        grid.contains(&at) && panel.contains(&at),
        "{grid:?} {panel:?}"
    );
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    assert!(cx.debug_bounds("shell-status-detail").is_none());
    assert!(view.read_with(cx, |this, _| this.handoff.selection.is_some()));
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
}

#[gpui::test]
fn status_detail_is_scoped_to_the_drawn_connection(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| {
        this.terminal.live.status = "Paste queued".into()
    });
    redraw(&view, cx);
    tap(cx, "shell-status-line");
    assert!(cx.debug_bounds("shell-status-detail").is_some());

    let (client, _) = mock_peer::connect_quiet().unwrap_or_else(|e| panic!("reconnect peer: {e}"));
    view.update(cx, |this, _| {
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    redraw(&view, cx);
    assert!(cx.debug_bounds("shell-status-detail-mark").is_some());
    assert!(cx.debug_bounds("shell-status-detail").is_none());

    tap(cx, "shell-status-line");
    assert!(cx.debug_bounds("shell-status-detail").is_some());
    view.update(cx, |this, _| {
        this.select_host("another-host");
        this.select_host("frame-overlay");
    });
    redraw(&view, cx);
    assert!(cx.debug_bounds("shell-status-detail").is_none());
    assert!(view.read_with(cx, |this, _| {
        this.shell.shortcuts.status_detail_open.is_none()
    }));
}

#[gpui::test]
fn shortcuts_draws_arrow_and_hardware_key_hint(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    tap(cx, "shell-status-shortcuts");
    assert_eq!(bounds(cx, "shell-shortcuts-arrow").size.height, px(10.));
    assert!(cx.debug_bounds("shell-shortcuts-hold-hint").is_none());

    view.update(cx, |this, _| this.shell.fit.hardware_keyboard = true);
    redraw(&view, cx);
    assert!(cx.debug_bounds("shell-shortcuts-hold-hint").is_some());
}

#[gpui::test]
fn copy_as_one_line_follows_copy_in_the_edit_bar(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| this.handoff.selection = Some(selection(this)));
    redraw(&view, cx);
    let copy = bounds(cx, "edit-bar-copy");
    let one_line = bounds(cx, "edit-bar-copy-one-line");
    let paste = bounds(cx, "edit-bar-paste");
    assert_eq!(one_line.left(), copy.right());
    assert_eq!(one_line.right(), paste.left());
    tap(cx, "edit-bar-copy-one-line");
    assert_eq!(
        view.read_with(cx, |this, _| this.handoff.clipboard_writes),
        1
    );
}

#[gpui::test]
fn picker_row_tap_uses_the_drawn_connection(cx: &mut TestAppContext) {
    let client = mock_peer::connect_board().unwrap_or_else(|e| panic!("board peer: {e}"));
    let (view, cx) = terminal_with_peer(cx, client);
    view.update(cx, |this, _| this.actions.board_hidden = true);
    redraw(&view, cx);

    tap(cx, "shell-space-switcher");
    let at = bounds(cx, "shell-space-picker-row-1").center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    view.update(cx, |this, _| {
        // Keep the boot and workspace IDs identical, with the generation
        // advancing between hit testing the press and dispatching its click.
        this.terminal.link.generation = this.terminal.link.generation.wrapping_add(1);
    });
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    assert!(view.read_with(cx, |this, _| this.terminal.navigating.is_none()));
    assert!(view.read_with(cx, |this, _| {
        this.terminal.live.status.contains("Stale board row")
    }));

    tap(cx, "shell-space-switcher");
    tap(cx, "shell-space-picker-row-1");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        snapshot(&this.terminal.live)
            .is_some_and(|shown| shown.focused_workspace_id.as_deref() == Some("w2"))
    }) {
        assert!(Instant::now() < deadline, "space picker did not focus w2");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_core(view: &Entity<Smoke>, cx: &mut VisualTestContext, ready: impl Fn(&Smoke) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        ready(this)
    }) {
        assert!(Instant::now() < deadline, "waiting for the Herdr 2 update");
        std::thread::sleep(Duration::from_millis(5));
    }
    redraw(view, cx);
}

#[gpui::test]
fn herdr2_chrome_dispatches_tab_actions_and_scrolls_above_the_keyboard(cx: &mut TestAppContext) {
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("mock core: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1210.), px(834.)));
    view.update(cx, |this, cx| {
        this.select_host("chrome-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait_for_core(&view, cx, |this| {
        this.terminal.live.connected && !this.row_tabs().is_empty()
    });
    assert_eq!(
        view.read_with(cx, |this, _| this.row_tabs()[0].1.clone()),
        "1 · sh"
    );
    assert!(cx.debug_bounds("shell-selected-tab-status-dot").is_none());
    assert!(cx.debug_bounds("shell-tab-underline").is_some());
    assert_eq!(
        bounds(cx, "shell-tab-0").right(),
        bounds(cx, "shell-new-tab").left()
    );

    tap(cx, "shell-new-tab");
    wait_for_core(&view, cx, |this| this.row_tabs().len() == 2);
    tap(cx, "shell-tab-body-0");
    wait_for_core(&view, cx, |this| this.row_tabs()[0].2);
    tap(cx, "shell-tab-close-1");
    wait_for_core(&view, cx, |this| this.row_tabs().len() == 1);

    tap(cx, "shell-more");
    tap(cx, "shell-overflow-item-Split down");
    wait_for_core(&view, cx, |this| {
        this.core_tabs()
            .iter()
            .any(|tab| tab.panes.len() == 2 && tab.vertical)
    });

    view.update(cx, |this, _| {
        this.shell.fit.insets = Some(WindowInsets {
            ime: gpui::Edges {
                bottom: px(320.),
                ..Default::default()
            },
            ..Default::default()
        });
    });
    redraw(&view, cx);
    tap(cx, "shell-more");
    let menu = bounds(cx, "shell-overflow-menu");
    assert!(menu.bottom() <= px(514.), "{menu:?}");
    let last = "shell-overflow-item-Toggle split direction";
    let before = bounds(cx, last);
    assert!(before.bottom() > menu.bottom());
    cx.simulate_event(ScrollEvent {
        position: menu.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-240.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Touch,
    });
    let after = bounds(cx, last);
    assert!(
        after.bottom() <= menu.bottom() && menu.contains(&after.center()),
        "{after:?}"
    );
    tap(cx, last);
    wait_for_core(&view, cx, |this| {
        this.core_tabs()
            .iter()
            .any(|tab| tab.panes.len() == 2 && !tab.vertical)
    });
}

/// The worker publishes the next catalog before the view takes the Live of
/// the same epoch. A frame drawn in between keeps the tabs of the Live it
/// shows: the strip neither disappears nor shows the newer selection.
#[gpui::test]
fn tab_strip_keeps_the_drawn_catalog_while_a_newer_one_is_published(cx: &mut TestAppContext) {
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("mock core: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1210.), px(834.)));
    view.update(cx, |this, cx| {
        this.select_host("chrome-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait_for_core(&view, cx, |this| {
        this.terminal.live.connected && !this.row_tabs().is_empty()
    });
    tap(cx, "shell-new-tab");
    wait_for_core(&view, cx, |this| this.row_tabs().len() == 2);
    let active = |cx: &mut VisualTestContext| {
        view.read_with(cx, |this, _| {
            this.row_tabs().iter().map(|tab| tab.2).collect::<Vec<_>>()
        })
    };
    assert_eq!(active(cx), [false, true]);

    tap(cx, "shell-tab-body-0");
    // Without polling, wait until the worker has published the next epoch:
    // its catalog goes out before the Live it puts in the inbox.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.read_with(cx, |this, _| {
        let drawn = this.terminal.live.input_epoch;
        this.terminal
            .link
            .inbox
            .lock()
            .is_ok_and(|published| published.input_epoch > drawn)
    }) {
        assert!(Instant::now() < deadline, "the worker did not publish");
        std::thread::sleep(Duration::from_millis(5));
    }
    redraw(&view, cx);
    assert!(cx.debug_bounds("shell-tab-0").is_some());
    assert!(cx.debug_bounds("shell-tab-1").is_some());
    assert_eq!(active(cx), [false, true]);

    wait_for_core(&view, cx, |this| this.row_tabs()[0].2);
    assert_eq!(active(cx), [true, false]);
}
