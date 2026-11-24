use super::*;

const ROW: f32 = 600.0;

/// Left edge of the pinned actions in a `ROW`-wide row.
fn pinned_left() -> f32 {
    ROW - pinned_width()
}

#[test]
fn tab_row_metrics() {
    let metrics = super::tab_row_metrics();
    assert_eq!(metrics.height, 36.0);
    assert_eq!(metrics.close_width, 32.0);
    assert_eq!(metrics.close_height, 36.0);
}

#[test]
fn close_hit_closes_and_does_not_select() {
    let tabs = [120.0, 140.0];
    // The last 32 pt of each tab is its × button.
    for x in [88.0, 100.0, 119.9] {
        assert_eq!(
            tab_hit(x, &tabs, ROW, 0.0),
            Some(TabHit::Close(0)),
            "x = {x}"
        );
    }
    for x in [228.0, 259.9] {
        assert_eq!(
            tab_hit(x, &tabs, ROW, 0.0),
            Some(TabHit::Close(1)),
            "x = {x}"
        );
    }
}

#[test]
fn tab_body_hit_selects_and_does_not_close() {
    let tabs = [120.0, 140.0];
    for x in [0.0, 40.0, 87.9] {
        assert_eq!(
            tab_hit(x, &tabs, ROW, 0.0),
            Some(TabHit::Select(0)),
            "x = {x}"
        );
    }
    for x in [120.0, 227.9] {
        assert_eq!(
            tab_hit(x, &tabs, ROW, 0.0),
            Some(TabHit::Select(1)),
            "x = {x}"
        );
    }
    // Past the last tab and its New tab button: nothing.
    assert_eq!(tab_hit(340.0, &tabs, ROW, 0.0), None);
}

#[test]
fn tab_actions_stay_pinned_when_tabs_overflow() {
    // Twelve tabs, far wider than the row: the strip scrolls under the
    // pinned actions, which keep the right edge.
    let tabs = [120.0; 12];
    let left = pinned_left();
    assert_eq!(tab_hit(left + 1.0, &tabs, ROW, 0.0), Some(TabHit::NewTab));
    assert_eq!(tab_hit(left + 45.0, &tabs, ROW, 0.0), Some(TabHit::Split));
    assert_eq!(tab_hit(ROW - 1.0, &tabs, ROW, 0.0), Some(TabHit::Overflow));
    assert!(matches!(
        tab_hit(left - 1.0, &tabs, ROW, 0.0),
        Some(TabHit::Select(_) | TabHit::Close(_))
    ));
    // The same three in a narrow Split View row.
    let narrow = 280.0;
    assert_eq!(
        tab_hit(narrow - 1.0, &tabs, narrow, 0.0),
        Some(TabHit::Overflow)
    );
    assert_eq!(
        tab_hit(narrow - pinned_width(), &tabs, narrow, 0.0),
        Some(TabHit::NewTab)
    );
}

#[test]
fn pinned_actions_not_pushed_out_by_long_tab_names() {
    // A tab whose name would measure 2,000 pt is capped, keeps its × inside
    // the cap, and never covers the pinned actions.
    let tabs = [2_000.0, 90.0];
    assert_eq!(
        tab_hit(MAX_TAB_WIDTH - 1.0, &tabs, ROW, 0.0),
        Some(TabHit::Close(0))
    );
    assert_eq!(
        tab_hit(MAX_TAB_WIDTH - CLOSE_WIDTH - 1.0, &tabs, ROW, 0.0),
        Some(TabHit::Select(0))
    );
    assert_eq!(
        tab_hit(MAX_TAB_WIDTH + 1.0, &tabs, ROW, 0.0),
        Some(TabHit::Select(1))
    );
    let right_actions_left = ROW - 2. * PINNED_ACTION_WIDTH;
    for (x, hit) in [
        (MAX_TAB_WIDTH + 90., TabHit::NewTab),
        (right_actions_left, TabHit::Split),
        (right_actions_left + PINNED_ACTION_WIDTH, TabHit::Overflow),
    ] {
        assert_eq!(tab_hit(x, &tabs, ROW, 0.0), Some(hit), "x = {x}");
    }
}

/// Window controls on the right (review of PR 87 at 16ef79e): with a 66 pt
/// trailing clearance in a 300 pt row, the model puts the pinned actions
/// where `render_tab_row` draws them, 66 pt from the edge, and the last
/// 66 pt hit nothing (the system's controls take those taps).
/// `frame_controls_tests.rs` checks the same against the drawn row.
#[test]
fn pinned_actions_end_before_window_controls_on_the_right() {
    let row = 300.0;
    let controls = 66.0;
    let tabs = [120.0; 12];
    let content_right = row - controls;
    for x in [content_right, row - 33.0, row - 1.0] {
        assert_eq!(tab_hit(x, &tabs, row, controls), None, "x = {x}");
    }
    assert_eq!(
        tab_hit(content_right - 1.0, &tabs, row, controls),
        Some(TabHit::Overflow)
    );
    let pinned_left = content_right - pinned_width();
    for (x, hit) in [
        (pinned_left, TabHit::NewTab),
        (pinned_left + PINNED_ACTION_WIDTH, TabHit::Split),
        (pinned_left + 2.0 * PINNED_ACTION_WIDTH, TabHit::Overflow),
    ] {
        assert_eq!(tab_hit(x, &tabs, row, controls), Some(hit), "x = {x}");
    }
    assert!(matches!(
        tab_hit(pinned_left - 1.0, &tabs, row, controls),
        Some(TabHit::Select(_) | TabHit::Close(_))
    ));
    // Positive control: without the clearance the old place is Overflow.
    assert_eq!(tab_hit(row - 1.0, &tabs, row, 0.0), Some(TabHit::Overflow));
}

/// Review S2: the × takes `run_action`'s fences. While Settings is open it
/// is refused before any other check.
#[gpui::test]
fn close_tab_refused_while_settings_open(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.settings_modal = true;
        let host = this.terminal.reconnect.target.clone();
        let generation = this.terminal.link.generation;
        let err = this
            .close_tab_by_id(&host, "tab", "boot", generation)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains("blocked by settings"), "{err}");
        assert!(this.terminal.navigating.is_none());
    });
}

/// Review S2: the × cancels the handoff like Close tab does, and a
/// suspended host refuses it without sending.
#[gpui::test]
fn close_tab_follows_run_action_fences(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.terminal.reconnect.suspended = true;
        let serial = this.handoff.serial;
        let host = this.terminal.reconnect.target.clone();
        let generation = this.terminal.link.generation;
        let err = this
            .close_tab_by_id(&host, "tab", "boot", generation)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains(HOST_UNAVAILABLE), "{err}");
        assert_ne!(this.handoff.serial, serial, "handoff not cancelled");
        assert!(this.handoff.selection.is_none());
        assert!(this.terminal.navigating.is_none());
    });
}

#[test]
fn selected_tab_has_tint_underline_dot_and_semibold() {
    let appearance = tab_appearance(true);
    assert_eq!(appearance.fill, Some(tint::SELECTED));
    assert_eq!(appearance.weight, weight::SEMIBOLD);
    assert_eq!(tab_appearance(false).text, color::MUTED);
    assert_eq!(metric::CURRENT_LINE, 2.);
    assert_eq!(metric::SMALL_DOT, 8.);
}

#[test]
fn tab_name_is_number_and_agent_or_process() {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(crate::mock_peer::SNAPSHOT)
        .unwrap_or_else(|error| panic!("snapshot: {error}"));
    snapshot.tabs[0].label = "1".into();
    assert_eq!(tab_name(&snapshot, &snapshot.tabs[0]), "1 · claude");
    snapshot.agents.clear();
    snapshot.panes[0].label = Some("zsh".into());
    assert_eq!(tab_name(&snapshot, &snapshot.tabs[0]), "1 · zsh");
    snapshot.tabs[0].custom_label = true;
    snapshot.tabs[0].label = "logs".into();
    assert_eq!(tab_name(&snapshot, &snapshot.tabs[0]), "1 · logs");
    snapshot.tabs[0].custom_label = false;
    snapshot.tabs[0].label = "1".into();
    snapshot.panes.clear();
    assert_eq!(tab_name(&snapshot, &snapshot.tabs[0]), "1");
}

#[test]
fn new_tab_button_follows_the_last_tab() {
    assert_eq!(tab_hit(89., &[90.], 600., 0.), Some(TabHit::Close(0)));
    assert_eq!(tab_hit(90., &[90.], 600., 0.), Some(TabHit::NewTab));
    assert_eq!(tab_hit(134., &[90.], 600., 0.), None);
    assert_eq!(tab_hit(512., &[90.], 600., 0.), Some(TabHit::Split));
}

/// One finger pan step of `x`, `y` points over `at` (issue #137 PR 2).
fn swipe(cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>, x: f32, y: f32) {
    cx.simulate_event(gpui::ScrollEvent {
        position: at,
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(x), px(y))),
        modifiers: gpui::Modifiers::none(),
        touch_phase: gpui::TouchPhase::Started,
        source: gpui::ScrollSource::Touch,
    });
    cx.run_until_parked();
}

/// Issue #137 PR 2: the overflowing tab strip moves sideways under a
/// sideways swipe only; a vertical swipe leaves it where it is.
#[gpui::test]
fn the_tab_strip_ignores_vertical_swipes(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(gpui::size(px(1200.), px(1000.)));
    let (client, _) =
        crate::mock_peer::connect_quiet().unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("tab-strip");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        !this.row_tabs().is_empty()
    }) {
        assert!(std::time::Instant::now() < deadline, "no tabs");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // Thirty more tabs overflow the strip.
    view.update(cx, |this, cx| {
        let Some(snapshot) = this.terminal.live.snapshot.as_mut() else {
            panic!("snapshot");
        };
        let snapshot = std::sync::Arc::make_mut(snapshot);
        let Some(first) = snapshot
            .tabs
            .iter()
            .find(|tab| Some(&tab.workspace_id) == snapshot.focused_workspace_id.as_ref())
            .cloned()
        else {
            panic!("a tab of the focused workspace");
        };
        for n in 0..30 {
            let mut tab = first.clone();
            tab.tab_id = format!("extra-{n}");
            tab.focused = false;
            snapshot.tabs.push(tab);
        }
        cx.notify();
    });
    cx.run_until_parked();
    let first = |cx: &mut gpui::VisualTestContext| {
        cx.debug_bounds("shell-tab-0")
            .unwrap_or_else(|| panic!("first tab"))
    };
    let at = first(cx).center();
    let start = first(cx).left();
    swipe(cx, at, 0., -80.);
    assert_eq!(first(cx).left(), start, "a vertical swipe moved the strip");
    swipe(cx, at, -80., 0.);
    assert!(
        first(cx).left() < start,
        "a sideways swipe must move the overflowing strip"
    );
}
