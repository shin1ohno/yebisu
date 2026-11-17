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
        assert_eq!(tab_hit(x, &tabs, ROW), Some(TabHit::Close(0)), "x = {x}");
    }
    for x in [228.0, 259.9] {
        assert_eq!(tab_hit(x, &tabs, ROW), Some(TabHit::Close(1)), "x = {x}");
    }
}

#[test]
fn tab_body_hit_selects_and_does_not_close() {
    let tabs = [120.0, 140.0];
    for x in [0.0, 40.0, 87.9] {
        assert_eq!(tab_hit(x, &tabs, ROW), Some(TabHit::Select(0)), "x = {x}");
    }
    for x in [120.0, 227.9] {
        assert_eq!(tab_hit(x, &tabs, ROW), Some(TabHit::Select(1)), "x = {x}");
    }
    // Past the last tab and before the pinned actions: nothing.
    assert_eq!(tab_hit(300.0, &tabs, ROW), None);
}

#[test]
fn tab_actions_stay_pinned_when_tabs_overflow() {
    // Twelve tabs, far wider than the row: the strip scrolls under the
    // pinned actions, which keep the right edge.
    let tabs = [120.0; 12];
    let left = pinned_left();
    assert_eq!(tab_hit(left + 1.0, &tabs, ROW), Some(TabHit::NewTab));
    assert_eq!(tab_hit(left + 45.0, &tabs, ROW), Some(TabHit::Split));
    assert_eq!(tab_hit(ROW - 1.0, &tabs, ROW), Some(TabHit::Overflow));
    assert!(matches!(
        tab_hit(left - 1.0, &tabs, ROW),
        Some(TabHit::Select(_) | TabHit::Close(_))
    ));
    // The same three in a narrow Split View row.
    let narrow = 280.0;
    assert_eq!(tab_hit(narrow - 1.0, &tabs, narrow), Some(TabHit::Overflow));
    assert_eq!(
        tab_hit(narrow - pinned_width(), &tabs, narrow),
        Some(TabHit::NewTab)
    );
}

#[test]
fn pinned_actions_not_pushed_out_by_long_tab_names() {
    // A tab whose name would measure 2,000 pt is capped, keeps its × inside
    // the cap, and never covers the pinned actions.
    let tabs = [2_000.0, 90.0];
    assert_eq!(
        tab_hit(MAX_TAB_WIDTH - 1.0, &tabs, ROW),
        Some(TabHit::Close(0))
    );
    assert_eq!(
        tab_hit(MAX_TAB_WIDTH - CLOSE_WIDTH - 1.0, &tabs, ROW),
        Some(TabHit::Select(0))
    );
    assert_eq!(
        tab_hit(MAX_TAB_WIDTH + 1.0, &tabs, ROW),
        Some(TabHit::Select(1))
    );
    let left = pinned_left();
    for (x, hit) in [
        (left, TabHit::NewTab),
        (left + PINNED_ACTION_WIDTH, TabHit::Split),
        (left + 2.0 * PINNED_ACTION_WIDTH, TabHit::Overflow),
    ] {
        assert_eq!(tab_hit(x, &tabs, ROW), Some(hit), "x = {x}");
    }
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
