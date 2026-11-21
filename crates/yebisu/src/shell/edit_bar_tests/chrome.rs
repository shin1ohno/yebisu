//! Rule: while selecting, the frame's controls stay usable. The bar adds its
//! Copy, Paste, Paste image and Select actions and never hides another
//! entry. A selection pauses typing while the command catalog stays usable.
use super::support::{selection, window};
use crate::{
    actions::catalog,
    shell::{
        ChromeState, EntryAction, HerdrKind,
        edit_bar::{edit_bar_entry_points, edit_bar_items},
    },
};

const FRAME_ACTIONS: [EntryAction; 6] = [
    EntryAction::Hosts,
    EntryAction::Settings,
    EntryAction::NewSpace,
    EntryAction::NewTab,
    EntryAction::Split,
    EntryAction::ToggleSidebar,
];

#[test]
fn chrome_stays_usable_while_selecting() {
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        let state = ChromeState {
            selecting: true,
            connected: true,
            kind,
            ..ChromeState::default()
        };
        let points = edit_bar_entry_points(&state);
        for item in edit_bar_items(true) {
            assert!(points.iter().any(|point| point.action == item.action()));
        }
        for action in FRAME_ACTIONS {
            assert!(
                points.iter().all(|p| p.action != action),
                "{action:?} is not the bar's to disable or replace"
            );
        }
    }
}

#[gpui::test]
fn the_command_catalog_ignores_the_selection(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |this, _| this.terminal.open = true);
    let snapshot = |cx: &mut gpui::VisualTestContext| {
        view.update(cx, |this, _| {
            catalog(&this.terminal)
                .into_iter()
                .map(|action| (action.label, action.available))
                .collect::<Vec<_>>()
        })
    };
    let idle = snapshot(cx);
    view.update(cx, |this, _| {
        this.handoff.select_mode = true;
        this.handoff.selection = Some(selection());
        assert!(this.is_selecting());
    });
    let selecting = snapshot(cx);
    assert_eq!(idle, selecting, "selection must not change availability");
    assert!(
        selecting.iter().any(|(_, available)| *available),
        "the control: the catalog offers something to compare"
    );
}
