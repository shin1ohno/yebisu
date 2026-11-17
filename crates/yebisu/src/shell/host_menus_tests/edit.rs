//! Host › Edit opens the host that was selected when the menu was shown.
use super::super::*;
use super::support::smoke;
use crate::native_menus::NativeCommand;
use gpui::TestAppContext;

#[test]
fn edit_target_is_the_shown_host() {
    let shown = MenuScope {
        host: "studio".into(),
        space: Some("w1".into()),
    };
    // A space change does not move a Host item's target.
    let current = MenuScope {
        host: "studio".into(),
        space: Some("w2".into()),
    };
    assert_eq!(edit_target(&shown, &current).as_deref(), Some("studio"));
    let other = MenuScope {
        host: "alpha".into(),
        space: None,
    };
    assert_eq!(edit_target(&shown, &other), None);
}

#[gpui::test]
fn menu_edit_host_targets_selected_host(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx, "studio");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Another saved host comes first; the menu must not pick it.
            this.setup.hosts = vec!["alpha".into(), "studio".into()];
            this.install_native_menus(window, cx);
            assert_eq!(this.native_menus.menu_scope().host, "studio");
            this.native_command_action(&NativeCommand::named("host.edit"), window, cx);
            assert_eq!(this.shell.shortcuts.edit_host.as_deref(), Some("studio"));
            // After a host switch the shown menu's Edit is refused.
            this.shell.shortcuts.edit_host = None;
            // Edit opened Settings (L7's sheet is not here yet), which took focus.
            assert!(this.settings_modal);
            this.settings_modal = false;
            this.terminal_focus.focus(window, cx);
            this.terminal.reconnect.target = "alpha".into();
            this.native_command_action(&NativeCommand::named("host.edit"), window, cx);
            assert_eq!(this.shell.shortcuts.edit_host, None);
            assert!(this.terminal.live.status.contains("menu target changed"));
        })
    });
}
