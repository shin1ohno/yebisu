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
fn herdr2_paste_image_is_enabled() {
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
    assert!(image.enabled);
    let menu = crate::shell::overflow_menu::overflow_menu_items(&state);
    assert!(menu.iter().any(|entry| {
        entry.item == crate::shell::overflow_menu::OverflowItem::PasteImage && entry.enabled
    }));
    assert_eq!(image.reason, None);
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

#[test]
fn edit_bar_has_shadow_medium_weight_and_cancel_icon() {
    use crate::shell::{
        edit_bar::edit_bar_surface,
        style::{shadow, weight},
        tab_row::icons::Icon,
    };
    use gpui::Styled;
    let mut bar = edit_bar_surface();
    let style = bar.style();
    assert_eq!(style.box_shadow, Some(vec![shadow::EDIT_BAR.box_shadow()]));
    assert_eq!(style.text.font_weight, Some(weight::MEDIUM));
    assert_eq!(Icon::Cancel.size(), (14., 14.));
}
