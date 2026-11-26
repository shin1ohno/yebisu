//! Issue #209: in a narrow frame with the sidebar hidden to the rail, the
//! tab row's pinned buttons (+ is not one of them: split and ...) stay out of
//! the room the row keeps after them for the window controls or the status
//! bar's icons, and inside the window, at every frame width, left to right
//! and right to left, with a short or a long space name.
//!
//! Before the fix the space switcher, + and the pinned buttons never shrank:
//! once their widths passed the row less its trailing room, the row's flex
//! layout pushed the pinned buttons into that room, and in full screen past
//! the window's edge (task 82ae4215a38a4b38 `measure-before.log`). Now the
//! switcher cuts its name to fit, and a row too short beside the status
//! icons stays below the band (owner decision A, 2026-11-26), as the compact
//! bar does (#208). Full screen and a Mac keep their look wherever the row
//! already fitted.
use super::overlay_tests::terminal;
use crate::{
    app::Smoke,
    layout::STATUS_ICONS_REACH,
    shell::tab_row::{space_switcher_max_width, tab_row_min_content},
};
use gpui::{
    Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets, px, size,
};
use std::sync::Arc;

const HEIGHT: f32 = 750.0;
/// The compact threshold up to the owner's external-display window, with
/// the widths the issue measured and the long-name thresholds around them.
/// 549 and 550 pt are the band's threshold beside the rail (`BAND_FROM`).
const WIDTHS: [f32; 17] = [
    352.0, 356.0, 359.0, 360.0, 370.0, 400.0, 414.0, 415.0, 450.0, 500.0, 519.0, 520.0, 549.0,
    550.0, 580.0, 1024.0, 1467.0,
];
/// The fixture's "Space 1" switcher, measured before the fix.
const SHORT_SWITCHER: f32 = 89.0;
/// The fixture's own space name and one that fills the switcher's 180 pt.
const LONG_NAME: &str = "build-and-release-pipeline";
/// Top safe areas: the owner's external-display window (status bar in the
/// band), a window clear of the screen's top edge, full screen, a Mac.
const WINDOW_TOP: f32 = 32.0;
const CLEAR_TOP: f32 = 10.0;
const FULL_SCREEN_TOP: f32 = 24.0;
/// herdr-uikit's samples (left, right), as in `frame_hidden_sidebar_tests.rs`.
const CONTROLS: (f32, f32) = (66.0, 9.5);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);
const MAC: (f32, f32) = (103.92, 0.0);

#[derive(Clone, Copy, Debug)]
struct Case {
    top: f32,
    reach: (f32, f32),
    traffic_lights: bool,
}

const CASES: [Case; 6] = [
    Case {
        top: WINDOW_TOP,
        reach: CONTROLS,
        traffic_lights: false,
    },
    Case {
        top: CLEAR_TOP,
        reach: CONTROLS,
        traffic_lights: false,
    },
    Case {
        top: WINDOW_TOP,
        reach: CONTROLS_RIGHT,
        traffic_lights: false,
    },
    Case {
        top: CLEAR_TOP,
        reach: CONTROLS_RIGHT,
        traffic_lights: false,
    },
    Case {
        top: FULL_SCREEN_TOP,
        reach: FULL_SCREEN,
        traffic_lights: false,
    },
    Case {
        top: 0.0,
        reach: MAC,
        traffic_lights: true,
    },
];

fn rename_spaces(view: &Entity<Smoke>, cx: &mut VisualTestContext, name: &str) {
    view.update(cx, |this, _| {
        let shown = this
            .terminal
            .live
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("fixture snapshot"));
        let mut renamed = (**shown).clone();
        for space in &mut renamed.workspaces {
            space.label = name.into();
        }
        this.terminal.live.snapshot = Some(Arc::new(renamed));
    });
}

fn draw(view: &Entity<Smoke>, cx: &mut VisualTestContext, width: f32, case: Case) {
    cx.simulate_resize(size(px(width), px(HEIGHT)));
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(WindowInsets {
            safe_area: Edges {
                top: px(case.top),
                ..Edges::default()
            },
            ..WindowInsets::default()
        });
        this.shell.fit.window_controls = Some(Edges {
            left: px(case.reach.0),
            right: px(case.reach.1),
            ..Edges::default()
        });
        this.shell.fit.traffic_lights_over_content = case.traffic_lights;
        this.actions.board_hidden = true;
        cx.notify();
    });
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

/// What the pinned buttons must keep clear of at the window's right edge in
/// `case`: the controls' reach on the right, the status icons while the row
/// sits in a band that holds the status bar, else nothing.
fn trailing_room(case: Case, row_top: f32) -> f32 {
    let (left, right) = case.reach;
    if case.traffic_lights {
        return 0.0;
    }
    if right - left > 12.0 {
        return right;
    }
    if left - right > 12.0 && case.top >= WINDOW_TOP && row_top < case.top {
        return STATUS_ICONS_REACH;
    }
    0.0
}

/// The narrowest a tab strip gets: a tab's padding and × where the row has
/// room for it, else what the switcher's minimum, + and the pinned buttons
/// leave.
const MIN_TAB_STRIP: f32 = 12.0 + 32.0;

#[gpui::test]
fn the_pinned_buttons_keep_their_trailing_room_in_a_narrow_frame(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    for name in [None, Some(LONG_NAME)] {
        if let Some(name) = name {
            rename_spaces(&view, cx, name);
        }
        for case in CASES {
            for width in WIDTHS {
                draw(&view, cx, width, case);
                let row = bounds(cx, "shell-tab-row");
                let pinned = bounds(cx, "shell-tab-pinned");
                let room = trailing_room(case, f32::from(row.top()));
                let at = format!("width {width}, {case:?}, name {name:?}");
                assert!(
                    pinned.right() <= px(width - room),
                    "split and ... end {:?}, past {} pt ({at})",
                    pinned.right(),
                    width - room
                );
                let switcher = bounds(cx, "shell-space-switcher");
                assert!(
                    switcher.left() >= row.left() && switcher.right() <= pinned.left(),
                    "the switcher {:?}..{:?} stays before split and ... at {:?} ({at})",
                    switcher.left(),
                    switcher.right(),
                    pinned.left()
                );
                let new_tab = bounds(cx, "shell-new-tab");
                assert!(
                    new_tab.right() <= pinned.left(),
                    "+ ends {:?}, before split and ... at {:?} ({at})",
                    new_tab.right(),
                    pinned.left()
                );
                // A tab stays on screen: the strip keeps a tab's minimum where
                // the switcher's minimum leaves room for it.
                let content = f32::from(row.size.width) - room;
                let strip = f32::from(bounds(cx, "shell-tab-strip").size.width);
                let floor = MIN_TAB_STRIP.min(content - space_switcher_min() - 3.0 * 44.0);
                assert!(
                    strip >= floor - 0.5,
                    "the tab strip is {strip} pt, under {floor} ({at})"
                );
            }
        }
    }
}

/// `space_switcher_max_width`'s floor: the switcher with its name cut away.
fn space_switcher_min() -> f32 {
    space_switcher_max_width(0.0)
}

/// Where the row goes in the owner's external-display window (status bar in
/// the band, controls on the left): into the band from the width at which
/// the switcher's full 180 pt, a tab, + and the pinned buttons fit beside the
/// status icons, below it under that width, and with the controls on the
/// right (whose room the band does not change) always in the band.
#[gpui::test]
fn a_row_too_short_beside_the_status_icons_stays_below_the_band(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    for width in WIDTHS {
        let window = CASES[0];
        draw(&view, cx, width, window);
        let row = bounds(cx, "shell-tab-row");
        let beside = width - f32::from(row.left()) - STATUS_ICONS_REACH;
        let expected_top = if beside >= tab_row_min_content(true) {
            0.0
        } else {
            WINDOW_TOP
        };
        assert_eq!(
            row.top(),
            px(expected_top),
            "the tab row's top at {width} pt ({beside} pt beside the icons)"
        );
        let right = Case {
            reach: CONTROLS_RIGHT,
            ..window
        };
        draw(&view, cx, width, right);
        assert_eq!(
            bounds(cx, "shell-tab-row").top(),
            px(0.0),
            "controls on the right: the row stays in the band at {width} pt"
        );
    }
    // The threshold beside the 72 pt rail and its rule.
    draw(&view, cx, 549.0, CASES[0]);
    assert_eq!(bounds(cx, "shell-tab-row").top(), px(WINDOW_TOP));
    draw(&view, cx, 550.0, CASES[0]);
    assert_eq!(bounds(cx, "shell-tab-row").top(), px(0.0));
    // The full sidebar (no switcher) keeps the row in the band from its
    // narrowest width.
    for width in [600.0, 1024.0] {
        draw(&view, cx, width, CASES[0]);
        view.update(cx, |this, cx| {
            this.actions.board_hidden = false;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("shell-space-switcher").is_none());
        assert_eq!(
            bounds(cx, "shell-tab-row").top(),
            px(0.0),
            "the full sidebar's row stays in the band at {width} pt"
        );
    }
}

/// Negative control: where the row already held the whole switcher, a
/// tab's minimum, + and the pinned buttons, nothing moves. In full screen,
/// on a Mac and in a window clear of the status bar the short name keeps its
/// whole switcher there, +, split and ... end at the window's edge, and the
/// row keeps its top, even with 30 tabs overflowing the strip. Narrower, the
/// switcher gives up only what a tab's minimum needs.
#[gpui::test]
fn full_screen_and_a_mac_keep_their_look(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    for tabs in [0, 30] {
        if tabs > 0 {
            add_tabs(&view, cx, tabs);
        }
        for case in [CASES[1], CASES[4], CASES[5]] {
            for width in WIDTHS {
                draw(&view, cx, width, case);
                let at = format!("width {width}, {case:?}, {tabs} more tabs");
                let switcher = bounds(cx, "shell-space-switcher");
                // The row's content starts past any leading room (a Mac's
                // traffic lights over the rail).
                let content = width - f32::from(switcher.left());
                let fitted = content - SHORT_SWITCHER - 3.0 * 44.0 >= MIN_TAB_STRIP;
                let expected = if fitted {
                    SHORT_SWITCHER
                } else {
                    space_switcher_max_width(content)
                };
                assert_eq!(
                    switcher.size.width,
                    px(expected),
                    "the switcher's width ({at}, fitted {fitted})"
                );
                assert_eq!(
                    bounds(cx, "shell-tab-pinned").right(),
                    px(width),
                    "split and ... end at the edge ({at})"
                );
                assert_eq!(
                    bounds(cx, "shell-tab-row").top(),
                    px(if case.traffic_lights || case.reach == FULL_SCREEN {
                        case.top
                    } else {
                        0.0
                    }),
                    "the row keeps its top ({at})"
                );
            }
        }
    }
}

/// Positive control for the cut: the long name's switcher is narrower than
/// its 180 pt in a narrow frame and back at 180 pt in a wide one.
#[gpui::test]
fn a_long_space_name_is_cut_in_a_narrow_frame(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    rename_spaces(&view, cx, LONG_NAME);
    draw(&view, cx, 352.0, CASES[4]);
    let narrow = bounds(cx, "shell-space-switcher").size.width;
    assert!(narrow < px(180.0), "cut at 352 pt: {narrow:?}");
    draw(&view, cx, 1024.0, CASES[4]);
    assert_eq!(bounds(cx, "shell-space-switcher").size.width, px(180.0));
}

fn add_tabs(view: &Entity<Smoke>, cx: &mut VisualTestContext, count: usize) {
    view.update(cx, |this, _| {
        let shown = this
            .terminal
            .live
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("fixture snapshot"));
        let mut more = (**shown).clone();
        let Some(first) = more
            .tabs
            .iter()
            .find(|tab| Some(&tab.workspace_id) == more.focused_workspace_id.as_ref())
            .cloned()
        else {
            panic!("a tab of the focused workspace");
        };
        for n in 0..count {
            let mut tab = first.clone();
            tab.tab_id = format!("extra-{n}");
            tab.focused = false;
            more.tabs.push(tab);
        }
        this.terminal.live.snapshot = Some(Arc::new(more));
    });
}
