use super::*;
use crate::layout::{FIND_BAR_HEIGHT, TAB_ROW_HEIGHT};

/// The three canvases plan v2 7.5 names: 13-inch, 11-inch, Split View half.
const WINDOWS: [Size; 3] = [
    Size::new(1376.0, 1032.0),
    Size::new(1210.0, 834.0),
    Size::new(605.0, 834.0),
];

fn insets(keyboard: f32) -> Insets {
    Insets {
        safe_top: 24.0,
        safe_bottom: 20.0,
        keyboard,
        ..Insets::default()
    }
}

#[test]
fn find_bar_renders_at_terminal_column_top() {
    let tab = COLUMN_ORDER.iter().position(|s| *s == ColumnSlot::TabRow);
    let find = COLUMN_ORDER.iter().position(|s| *s == ColumnSlot::FindBar);
    let terminal = COLUMN_ORDER.iter().position(|s| *s == ColumnSlot::Terminal);
    assert_eq!((tab, find, terminal), (Some(0), Some(1), Some(2)));
    for window in WINDOWS {
        for drawn in [true, false] {
            let layout = layout::compute(&frame_input(window, insets(0.0), true));
            let g = frame_geometry(&layout, drawn);
            let Some(term) = g.terminal else {
                panic!("{window:?} fits");
            };
            assert_eq!(g.find_bar.height, FIND_BAR_HEIGHT);
            assert_eq!(g.find_bar.y, g.tab_row.bottom() + RULE, "{window:?}");
            assert_eq!(term.y, g.find_bar.bottom(), "{window:?}");
            assert_eq!((g.find_bar.x, g.find_bar.width), (term.x, term.width));
            // Closed: no height, the terminal starts under the tab row.
            let closed = layout::compute(&frame_input(window, insets(0.0), false));
            let g = frame_geometry(&closed, drawn);
            assert_eq!(g.find_bar.height, 0.0);
            assert_eq!(g.terminal.map(|r| r.y), Some(g.tab_row.bottom() + RULE));
        }
    }
}

#[test]
fn terminal_receives_allocated_rect() {
    for window in WINDOWS {
        for keyboard in [0.0, 320.0] {
            let layout = layout::compute(&frame_input(window, insets(keyboard), false));
            let Some(allocated) = layout.terminal.rect() else {
                panic!("{window:?} fits");
            };
            let g = frame_geometry(&layout, true);
            assert_eq!(g.terminal, Some(allocated), "{window:?} {keyboard}");
            assert_eq!(g.sidebar, Some(layout.sidebar));
            // The frame's fixed rows add up to the window: tab row, rule,
            // terminal, rule, status bar, then the bottom inset.
            let used = TAB_ROW_HEIGHT + RULE + allocated.height + RULE + g.status_bar.height;
            let bottom = keyboard.max(20.0);
            assert_eq!(24.0 + used + bottom, window.height, "{window:?} {keyboard}");
            // An empty sidebar column hands its width to the terminal.
            let wide = frame_geometry(&layout, false);
            let Some(rect) = wide.terminal else {
                panic!("{window:?} fits");
            };
            assert_eq!(rect.x, layout.sidebar.x);
            assert_eq!(rect.right(), allocated.right());
            assert_eq!((rect.y, rect.height), (allocated.y, allocated.height));
            assert_eq!(wide.sidebar, None);
        }
    }
}

#[gpui::test]
fn forced_rail_reaches_the_entry_model(cx: &mut gpui::TestAppContext) {
    use crate::shell::{
        EntryAction, Place, entry_move::LegacyBands, entry_points::chrome_entry_points,
    };
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        let rail_actions = |this: &Smoke| {
            let state = this.chrome_state();
            let found: Vec<(EntryAction, bool, Option<&'static str>)> =
                chrome_entry_points(&state, LegacyBands::current())
                    .into_iter()
                    .filter(|e| e.place == Place::Rail)
                    .map(|e| (e.action, e.enabled, e.reason))
                    .collect();
            (state.sidebar_hidden, found)
        };
        // 300 on screen: the rail offers nothing.
        assert_eq!(rail_actions(this), (false, vec![]));
        // The width forces the rail while the user still wants 300.
        this.shell.frame.sidebar_collapsed = true;
        let (hidden, found) = rail_actions(this);
        assert!(hidden);
        for action in [EntryAction::Hosts, EntryAction::Settings] {
            assert!(
                found.contains(&(action, true, None)),
                "{action:?} must be reachable in the rail: {found:?}"
            );
        }
        // Integration 2 (L4 review F1): the toggle cannot change anything
        // while the width forces the rail, so it is off with its reason.
        assert!(
            found.contains(&(EntryAction::ToggleSidebar, false, Some("Window too narrow"))),
            "the forced rail's toggle must be disabled with its reason: {found:?}"
        );
        // The user chose the rail in a wide window.
        this.shell.frame.sidebar_collapsed = false;
        this.actions.board_hidden = true;
        assert!(rail_actions(this).0);
    });
}

/// Integration 2 (L4 review F1): while the width forces the rail, "Show
/// sidebar ⌘B" is not offered, Command-B and the palette row refuse with
/// "Window too narrow" and the saved choice stays; once the window is wide
/// enough the same command switches the sidebar again.
#[gpui::test]
fn forced_rail_refuses_the_sidebar_toggles_with_a_reason(cx: &mut gpui::TestAppContext) {
    use crate::shell::{
        EntryAction, Place, entry_move::LegacyBands, entry_points::chrome_entry_points,
        status_bar::status_bar_model,
    };
    use sheprd_commands::Command;
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        // A Herdr 1 terminal screen: the catalog offers its local commands.
        this.terminal.open = true;
        this.shell.frame.sidebar_collapsed = true;
        let state = this.chrome_state();
        assert!(state.sidebar_hidden && state.sidebar_forced);
        assert!(!status_bar_model(&state, "h").show_sidebar_hint);
        let toggles: Vec<_> = chrome_entry_points(&state, LegacyBands::current())
            .into_iter()
            .filter(|e| e.action == EntryAction::ToggleSidebar)
            .collect();
        assert!(
            toggles.iter().all(|e| e.place != Place::StatusBar),
            "the status bar must not offer Show sidebar: {toggles:?}"
        );
        assert!(
            toggles
                .iter()
                .all(|e| !e.enabled && e.reason == Some("Window too narrow")),
            "every sidebar toggle is off with its reason: {toggles:?}"
        );
        let Err(error) = this.run_action(Command::ToggleSidebar) else {
            panic!("Command-B must refuse while the width forces the rail");
        };
        assert!(
            error.to_string().contains("Window too narrow"),
            "the refusal says why: {error}"
        );
        assert!(!this.actions.board_hidden, "the saved choice stays 300");
        // Positive control: in a wide window the same command switches.
        this.shell.frame.sidebar_collapsed = false;
        assert!(!this.chrome_state().sidebar_forced);
        this.run_action(Command::ToggleSidebar)
            .unwrap_or_else(|e| panic!("toggle in a wide window: {e}"));
        assert!(this.actions.board_hidden, "Command-B chose the rail");
    });
}
