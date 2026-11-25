//! Where a rename starts (issue AC1-AC3): the long-press menus, a double
//! tap, the palette and the menu bar; Rename on another space's row.
use super::*;
use crate::shell::{
    ChromeState,
    host_menus::{
        self, MenuItemId, MenuScope, SpaceMenuItem, long_press_label, menu_item_label,
        rename_scope_valid, space_long_press_for,
    },
};
use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, point};
use sheprd_commands::Command;

const RENAME: MenuItemId = MenuItemId::Space(SpaceMenuItem::Rename);

fn connected() -> ChromeState {
    ChromeState {
        connected: true,
        ..ChromeState::default()
    }
}

fn scope(host: &str, space: &str) -> MenuScope {
    MenuScope {
        host: host.into(),
        space: Some(space.into()),
    }
}

#[test]
fn long_press_lists_rename_everywhere() {
    let entries = host_menus::space_long_press(&connected());
    assert!(entries.iter().any(|e| e.item == RENAME && e.enabled));
    assert_eq!(long_press_label(RENAME, "session-search"), "Rename…");
}

#[test]
fn long_press_label_is_rename_ellipsis() {
    assert_eq!(long_press_label(RENAME, "x"), "Rename…");
    assert_eq!(
        long_press_label(MenuItemId::Space(SpaceMenuItem::NewTab), "setup"),
        "New tab — setup"
    );
}

#[test]
fn rename_space_label_has_target() {
    assert_eq!(
        menu_item_label(RENAME, "Space 2 · session-search"),
        "Rename Space — Space 2 · session-search"
    );
}

#[test]
fn rename_enabled_on_another_space_row() {
    let entries = space_long_press_for(&connected(), &scope("A", "w2"), &scope("A", "w1"));
    let rename = entries.iter().find(|e| e.item == RENAME);
    assert!(
        rename.is_some_and(|e| e.enabled),
        "Rename on another space's row: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .filter(|e| e.item != RENAME)
            .all(|e| !e.enabled && e.reason == Some("Open this space first")),
        "the other items still need their space open"
    );
}

#[test]
fn rename_disabled_on_another_host_row() {
    let entries = space_long_press_for(&connected(), &scope("B", "w1"), &scope("A", "w1"));
    let rename = entries.iter().find(|e| e.item == RENAME);
    assert!(rename.is_some_and(|e| !e.enabled && e.reason == Some("Select this host first")));
}

#[test]
fn rename_pick_refused_after_space_removed() {
    assert!(rename_scope_valid(
        &scope("A", "w2"),
        &scope("A", "w1"),
        true
    ));
    assert!(!rename_scope_valid(
        &scope("A", "w2"),
        &scope("A", "w1"),
        false
    ));
    assert!(!rename_scope_valid(
        &scope("A", "w2"),
        &scope("B", "w1"),
        true
    ));
}

#[gpui::test]
fn palette_lists_three_renames(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.read_with(cx, |this, _| {
        let rows = crate::actions::catalog(&this.terminal);
        for command in [
            Command::RenameTab,
            Command::RenameWorkspace,
            Command::RenamePane,
        ] {
            assert!(
                rows.iter()
                    .any(|row| row.info.command == command && row.available),
                "{command:?} is not in the palette"
            );
        }
        let labels: Vec<_> = rows.iter().map(|row| row.label.clone()).collect();
        for label in ["Rename Tab", "Rename Space", "Rename Pane"] {
            assert!(
                labels.iter().any(|l| l == label),
                "{label} missing: {labels:?}"
            );
        }
    });
}

#[gpui::test]
fn palette_rename_opens_the_field(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    view.update(cx, |this, cx| {
        this.actions.palette = true;
        let ran = this.run_action(Command::RenameTab);
        assert!(ran.is_ok(), "{ran:?}");
        assert!(!this.actions.palette, "the palette closes");
        cx.notify();
    });
    draw(&view, cx);
    assert_eq!(fake.presents(), 1, "Rename Tab did not present the field");
    view.read_with(cx, |this, _| {
        let session = this
            .shell
            .rename
            .session
            .as_ref()
            .unwrap_or_else(|| panic!("session"));
        assert_eq!(session.anchor, RenameAnchor::TabLabel);
        assert_eq!(session.target.object, tab("w1:t1"));
    });
}

#[gpui::test]
fn rename_space_only_in_space_menu(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.read_with(cx, |this, _| {
        let rows = crate::native_menus::catalog_rows(&this.terminal, &this.actions, true);
        let commands: Vec<_> = rows.iter().map(|row| row.info.command).collect();
        assert!(commands.contains(&Command::RenameTab));
        assert!(commands.contains(&Command::RenamePane));
        assert!(
            !commands.contains(&Command::RenameWorkspace),
            "Rename Space is duplicated in the Terminal menu"
        );
        let space = host_menus::menu_bar(&this.menu_chrome_state(), true);
        assert!(
            space
                .iter()
                .any(|(title, entries)| *title == host_menus::SPACE_MENU
                    && entries.iter().any(|e| e.item == RENAME))
        );
    });
}

fn double_click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not painted"));
    let position = point(
        at.origin.x + at.size.width / 2.,
        at.origin.y + at.size.height / 2.,
    );
    for click_count in [1, 2] {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count,
        });
        cx.run_until_parked();
    }
}

#[gpui::test]
fn double_tap_tab_moves_once_then_opens(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    methods(&view, &receipts, cx);
    double_click(cx, "shell-tab-body-0");
    draw(&view, cx);
    let moves: Vec<_> = methods(&view, &receipts, cx)
        .into_iter()
        .filter(|m| m == "tab.focus")
        .collect();
    assert!(moves.len() <= 1, "the second tap moved again: {moves:?}");
    assert_eq!(fake.presents(), 1, "the second tap did not open the field");
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.shell.rename.session.as_ref().map(|s| s.anchor),
            Some(RenameAnchor::TabLabel)
        );
    });
}

#[gpui::test]
fn double_tap_space_moves_once(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    methods(&view, &receipts, cx);
    double_click(cx, "sidebar-space-name-A-w2");
    draw(&view, cx);
    let moves: Vec<_> = methods(&view, &receipts, cx)
        .into_iter()
        .filter(|m| m == "workspace.focus")
        .collect();
    assert_eq!(moves.len(), 1, "the space row moved {} times", moves.len());
    assert_eq!(fake.presents(), 1, "the second tap did not open the field");
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.shell
                .rename
                .session
                .as_ref()
                .map(|s| s.target.object.clone()),
            Some(space("w2"))
        );
    });
}
