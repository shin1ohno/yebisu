//! Herdr 2 (issue AC15): Rename is listed, disabled, with
//! "Not available on Herdr 2 yet"; nothing is sent.
use super::*;
use crate::shell::{ChromeState, HerdrKind, host_menus, rename::HERDR2};
use sheprd_commands::Command;

#[test]
fn disabled_with_reason_in_the_space_menu() {
    let state = ChromeState {
        kind: HerdrKind::Herdr2,
        connected: true,
        ..ChromeState::default()
    };
    let menus = host_menus::menu_bar(&state, true);
    let rename = menus
        .iter()
        .flat_map(|(_, entries)| entries)
        .find(|e| e.item == host_menus::MenuItemId::Space(host_menus::SpaceMenuItem::Rename));
    assert!(rename.is_some_and(|e| !e.enabled && e.reason == Some(HERDR2)));
}

#[test]
fn subtitles_only_for_disabled_herdr2() {
    let rows = vec![
        ("rename_tab".to_owned(), "Rename Tab".to_owned(), false),
        ("rename_pane".to_owned(), "Rename Pane".to_owned(), false),
        ("next_tab".to_owned(), "Next Tab".to_owned(), false),
    ];
    let host = vec![(
        host_menus::SPACE_MENU,
        vec![(
            "space.rename".to_owned(),
            "Rename Space — Space 1".to_owned(),
            false,
        )],
    )];
    let subtitles = crate::native_menus::rename_subtitles(&rows, &host, true);
    assert_eq!(
        subtitles,
        [
            ("Rename Tab".to_owned(), HERDR2.to_owned()),
            ("Rename Pane".to_owned(), HERDR2.to_owned()),
            ("Rename Space — Space 1".to_owned(), HERDR2.to_owned()),
        ]
    );
    assert!(crate::native_menus::rename_subtitles(&rows, &host, false).is_empty());
}

/// The palette's disabled Rename rows say why (controller: a test that
/// fails without the line).
#[gpui::test]
fn palette_row_says_why(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.update(cx, |this, _| {
        assert_eq!(this.palette_reason(Command::RenameTab, true), None);
        assert_eq!(this.palette_reason(Command::RenameTab, false), None, "Herdr 1 has no reason");
        this.terminal.core_mode = true;
        for command in [Command::RenameTab, Command::RenameWorkspace, Command::RenamePane] {
            assert_eq!(this.palette_reason(command, false), Some(HERDR2), "{command:?}");
        }
        assert_eq!(this.palette_reason(Command::NextTab, false), None);
        this.terminal.core_mode = false;
    });
}

#[gpui::test]
fn no_request(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    view.update(cx, |this, _| {
        let mut target = this
            .rename_target(space("w1"))
            .unwrap_or_else(|| panic!("fixture: target"));
        target.kind = HerdrKind::Herdr2;
        this.begin_rename(target, RenameAnchor::SpaceRow);
        assert!(this.shell.rename.session.is_none(), "a Herdr 2 rename started");
        assert_eq!(this.terminal.live.status, HERDR2);
    });
    draw(&view, cx);
    assert_eq!(fake.presents(), 0);
    assert!(renames(&view, &receipts, cx).is_empty());
}
