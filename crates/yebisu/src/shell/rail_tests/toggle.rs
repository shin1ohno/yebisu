use crate::{
    actions::catalog,
    app::Smoke,
    shell::rail::{SidebarMode, sidebar_mode},
};
use gpui::TestAppContext;
use sheprd_commands::Command;

#[gpui::test]
fn toggle_sidebar_switches_rail(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        assert_eq!(this.sidebar_user_mode(), SidebarMode::Full);
        assert_eq!(
            sidebar_mode(1376.0, this.sidebar_user_mode()),
            SidebarMode::Full
        );
        // Command-B and the sidebar button: 300 to 72.
        this.toggle_sidebar();
        assert_eq!(this.sidebar_user_mode(), SidebarMode::Rail);
        assert_eq!(
            sidebar_mode(1376.0, this.sidebar_user_mode()),
            SidebarMode::Rail
        );
        // The saved flag is the same state, so a restart keeps the rail.
        assert!(this.actions.board_hidden);
        // And back, 72 to 300.
        this.toggle_sidebar();
        assert_eq!(this.sidebar_user_mode(), SidebarMode::Full);
        assert_eq!(
            sidebar_mode(1376.0, this.sidebar_user_mode()),
            SidebarMode::Full
        );
        // In a window too narrow for 300 the choice still toggles; the
        // window decides what is drawn.
        this.toggle_sidebar();
        this.toggle_sidebar();
        assert_eq!(
            sidebar_mode(480.0, this.sidebar_user_mode()),
            SidebarMode::Rail
        );
    });
}

#[gpui::test]
fn toggle_sidebar_stays_local_command(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        // PR 44's oracle: no host, no snapshot, no link; the command still
        // runs because it never leaves the app.
        this.terminal.open = true;
        assert!(!this.terminal.live.connected);
        let entry = catalog(&this.terminal)
            .into_iter()
            .find(|a| a.info.command == Command::ToggleSidebar);
        assert!(entry.is_some_and(|a| a.available));
        assert!(this.terminal.link.handle.is_none());
        assert!(this.run_action(Command::ToggleSidebar).is_ok());
        assert_eq!(this.sidebar_user_mode(), SidebarMode::Rail);
        assert!(this.terminal.navigating.is_none());
        assert!(this.terminal.live.pending_request.is_none());
        // Command Palette is the other local command and is untouched.
        assert!(!this.actions.palette);
    });
}
