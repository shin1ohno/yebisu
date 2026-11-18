//! Long press of a host or space row opens that row's Host or Space items
//! (plan v2 7.11, Q8), bound to the pressed row (review S4).
use crate::shell::sidebar::long_press::PressedRow;
use crate::{
    app::Smoke,
    shell::host_menus::{self, HostMenuItem, MenuItemId, SpaceMenuItem},
};
use gpui::{AppContext, TestAppContext, point, px};

#[gpui::test]
fn long_press_opens_the_rows_items_bound_to_the_row(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.select_host("alpha");
            let at = point(px(40.), px(120.));
            // The selected host's row lists the Host items.
            this.sidebar_long_press(PressedRow::Host("alpha".into()), at);
            let state = this.menu_chrome_state();
            let menu = this.shell.sidebar.long_press.clone();
            assert!(menu.as_ref().is_some_and(|m| m.shown.host == "alpha"));
            assert_eq!(
                menu.map(|m| m.entries),
                Some(host_menus::host_long_press(&state))
            );
            // Disconnected, Edit runs on the pressed (selected) host.
            let edit = MenuItemId::Host(HostMenuItem::Edit);
            assert!(this.pick_long_press(edit, window, cx).is_ok());
            assert_eq!(this.shell.shortcuts.edit_host.as_deref(), Some("alpha"));
            assert!(this.shell.sidebar.long_press.is_none(), "a pick closes it");
            this.shell.hosts_sheet.close();
            // Another host's row: its items would run on "alpha", so they
            // are disabled and a pick is refused.
            this.shell.shortcuts.edit_host = None;
            this.sidebar_long_press(PressedRow::Host("beta".into()), at);
            let menu = this.shell.sidebar.long_press.clone();
            assert!(
                menu.is_some_and(|m| m.entries.iter().all(|e| !e.enabled && e.reason.is_some()))
            );
            assert!(this.pick_long_press(edit, window, cx).is_err());
            assert_eq!(this.shell.shortcuts.edit_host, None);
            // A space row of the selected host that is not the focused space.
            this.sidebar_long_press(
                PressedRow::Space {
                    host: "alpha".into(),
                    id: "w9".into(),
                    name: "Space 9 · other".into(),
                },
                at,
            );
            let menu = this.shell.sidebar.long_press.clone();
            assert!(
                menu.is_some_and(|m| m.shown.space.as_deref() == Some("w9")
                    && m.entries.iter().all(|e| !e.enabled))
            );
            let new_tab = MenuItemId::Space(SpaceMenuItem::NewTab);
            assert!(this.pick_long_press(new_tab, window, cx).is_err());
            assert!(this.terminal.navigating.is_none());
        })
    });
}
