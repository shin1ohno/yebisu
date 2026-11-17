//! Rule: the edit bar exists only while a selection pauses typing.
use super::support::{selection, window};
use crate::shell::{
    ChromeState, EntryAction, HerdrKind, Place,
    edit_bar::{EditBarItem, edit_bar_entry_points, edit_bar_items},
};

#[test]
fn edit_bar_only_while_selecting() {
    assert!(edit_bar_items(false).is_empty());
    assert_eq!(
        edit_bar_items(true),
        [
            EditBarItem::Copy,
            EditBarItem::Paste,
            EditBarItem::PasteImage,
            EditBarItem::Cancel
        ]
    );
    let idle = ChromeState::default();
    assert!(edit_bar_entry_points(&idle).is_empty());
    let selecting = ChromeState {
        selecting: true,
        connected: true,
        ..ChromeState::default()
    };
    let points = edit_bar_entry_points(&selecting);
    assert_eq!(
        points.iter().map(|p| p.action).collect::<Vec<_>>(),
        [
            EntryAction::Copy,
            EntryAction::Paste,
            EntryAction::PasteImage,
            EntryAction::Select
        ]
    );
    assert!(
        points
            .iter()
            .all(|p| p.place == Place::EditBar && p.enabled)
    );
}

#[test]
fn paste_image_is_disabled_with_a_reason_on_herdr_2() {
    let state = ChromeState {
        selecting: true,
        connected: true,
        kind: HerdrKind::Herdr2,
        ..ChromeState::default()
    };
    let points = edit_bar_entry_points(&state);
    let image = points
        .iter()
        .find(|p| p.action == EntryAction::PasteImage)
        .unwrap_or_else(|| panic!("Paste image entry"));
    assert!(!image.enabled);
    assert_eq!(image.reason, Some("Herdr 2 accepts text only"));
    assert!(
        points
            .iter()
            .filter(|p| p.action != EntryAction::PasteImage)
            .all(|p| p.enabled && p.reason.is_none())
    );
}

#[gpui::test]
fn the_view_draws_the_bar_only_while_typing_is_paused(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    let drawn = |cx: &mut gpui::VisualTestContext| {
        view.update(cx, |this, cx| this.render_edit_bar(cx).is_some())
    };
    assert!(!drawn(cx), "idle");
    view.update(cx, |this, _| this.handoff.select_mode = true);
    assert!(drawn(cx), "Select mode without a selection yet");
    view.update(cx, |this, _| {
        this.handoff.select_mode = false;
        this.handoff.selection = Some(selection());
    });
    assert!(drawn(cx), "a selection from a drag or double tap");
    view.update(cx, |this, _| this.handoff.passive_highlight = true);
    assert!(!drawn(cx), "a highlight kept after typing resumed");
    view.update(cx, |this, _| {
        this.handoff.passive_highlight = false;
        this.handoff.selection = None;
    });
    assert!(!drawn(cx), "selection gone");
}
