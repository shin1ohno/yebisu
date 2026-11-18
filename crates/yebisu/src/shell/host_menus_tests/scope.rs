//! S-10: a menu pick whose host or space changed since it was shown is
//! refused.
use super::super::*;
use super::support::smoke;
use gpui::TestAppContext;

fn scope(host: &str, space: Option<&str>) -> MenuScope {
    MenuScope {
        host: host.into(),
        space: space.map(Into::into),
    }
}

#[test]
fn host_menu_item_refused_after_host_switch() {
    let shown = scope("alpha", None);
    assert!(scope_still_valid(&shown, &scope("alpha", Some("w1"))));
    assert!(!scope_still_valid(&shown, &scope("beta", Some("w1"))));
    assert!(!scope_still_valid(&scope("", None), &scope("", None)));
}

#[gpui::test]
fn app_refuses_host_item_after_host_switch(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx, "alpha");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Edit runs only without a live terminal until A6 (S2).
            this.terminal.live.connected = false;
            let shown = this.current_menu_scope();
            this.terminal.reconnect.target = "beta".into();
            let edit = MenuItemId::Host(HostMenuItem::Edit);
            let refused = this.run_menu_item(edit, &shown, window, cx);
            assert!(refused.is_err_and(|e| e.to_string().contains("menu target changed")));
            assert_eq!(this.shell.shortcuts.edit_host, None);
            // Positive control: back on the shown host the same pick runs.
            this.terminal.reconnect.target = "alpha".into();
            assert!(this.run_menu_item(edit, &shown, window, cx).is_ok());
            assert_eq!(this.shell.shortcuts.edit_host.as_deref(), Some("alpha"));
        })
    });
}

#[test]
fn space_menu_item_refused_after_space_change() {
    let shown = scope("alpha", Some("w1"));
    assert!(scope_still_valid(&shown, &scope("alpha", Some("w1"))));
    assert!(!scope_still_valid(&shown, &scope("alpha", Some("w2"))));
    assert!(!scope_still_valid(&shown, &scope("alpha", None)));
    assert!(!scope_still_valid(&shown, &scope("beta", Some("w1"))));
}

#[gpui::test]
fn app_refuses_space_item_after_space_change(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx, "alpha");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let current = this.current_menu_scope();
            assert!(current.space.is_some(), "fixture has a focused space");
            let shown = MenuScope {
                space: Some("another-space".into()),
                ..current.clone()
            };
            let new_tab = MenuItemId::Space(SpaceMenuItem::NewTab);
            let refused = this.run_menu_item(new_tab, &shown, window, cx);
            assert!(refused.is_err_and(|e| e.to_string().contains("menu target changed")));
            // Positive control: the current space passes the scope check and
            // reaches the catalog command (which has no wire in this test).
            let reached = this.run_menu_item(new_tab, &current, window, cx);
            assert!(reached.is_err_and(|e| !e.to_string().contains("menu target changed")));
        })
    });
}
