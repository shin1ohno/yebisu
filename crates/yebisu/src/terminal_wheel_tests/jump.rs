//! Issue #137 PR 5 (C2): Jump to bottom on Herdr 1, T28 to T30.
use super::*;

fn button(cx: &mut VisualTestContext, index: usize) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(Box::leak(
        format!("terminal-jump-bottom-{index}").into_boxed_str(),
    ))
}

fn scrolled_pane(id: &str, x: u16, width: u16, offset: u64) -> PaneSurfacePane {
    let mut pane = pane(id, x, width, 24);
    pane.scroll = Some(PaneSurfaceScrollMetrics {
        offset_from_bottom: offset,
        max_offset_from_bottom: 500,
        viewport_rows: 24,
    });
    pane
}

fn shown(view: &Entity<Smoke>, cx: &mut VisualTestContext, panes: Vec<PaneSurfacePane>) {
    install(view, cx, panes);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// T28: the button shows only while the pane shows its scrollback, at the
/// pane's lower right.
#[gpui::test]
fn jump_to_bottom_shows_only_while_scrolled_back(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    shown(&view, cx, vec![scrolled_pane(FOCUSED, 0, 80, 0)]);
    assert_eq!(button(cx, 0), None, "at the bottom");
    shown(&view, cx, vec![scrolled_pane(FOCUSED, 0, 80, 120)]);
    let Some(at) = button(cx, 0) else {
        panic!("scrolled back 120 rows");
    };
    let body = view.read_with(cx, |this, _| this.scrolled_back_panes());
    assert_eq!(body.len(), 1);
    let body = body[0].1;
    assert_eq!(at.right(), body.right() - px(16.));
    assert_eq!(at.bottom(), body.bottom() - px(12.));
    assert_eq!(at.size.height, px(32.));
    shown(&view, cx, vec![scrolled_pane(FOCUSED, 0, 80, 0)]);
    assert_eq!(button(cx, 0), None, "gone with the bottom surface");
}

/// T29: a tap asks Herdr for that pane's bottom, the unfocused one's too.
#[gpui::test]
fn a_tap_returns_its_own_pane_to_the_bottom(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected_with(cx, mock_peer::connect_quiet_scroll);
    shown(
        &view,
        cx,
        vec![
            scrolled_pane(FOCUSED, 0, 40, 0),
            scrolled_pane(OTHER, 41, 39, 50),
        ],
    );
    ready(&view, cx);
    let Some(at) = button(cx, 0) else {
        panic!("the other pane's button");
    };
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        pane_scrolls(&messages(&view, cx, &wire)),
        [serde_json::json!({"pane_id": OTHER, "offset_from_bottom": 0})]
    );
}

/// T30: the button is in front: a swipe or a press on it never reaches the
/// terminal under it.
#[gpui::test]
fn the_button_keeps_swipes_and_presses_from_the_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    shown(&view, cx, vec![scrolled_pane(FOCUSED, 0, 80, 120)]);
    let Some(at) = button(cx, 0) else {
        panic!("button");
    };
    let row = cell_height(&view, cx);
    scroll(
        &view,
        cx,
        at.center(),
        pixels(0., 3. * row),
        TouchPhase::Started,
    );
    assert_eq!(wheels(&view, cx, &wire), [], "a swipe on the button");
    cx.simulate_mouse_down(at.center(), gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at.center(), gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(
            this.handoff.selection.is_none(),
            "a press on the button selected"
        );
    });
}
