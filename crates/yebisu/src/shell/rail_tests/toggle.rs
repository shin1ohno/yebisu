use crate::{
    actions::catalog,
    app::Smoke,
    shell::rail::{SidebarMode, expand_unavailable, sidebar_mode},
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
        assert_eq!(
            sidebar_mode(480.0, this.sidebar_user_mode()),
            SidebarMode::Rail
        );
    });
}

#[gpui::test]
fn expand_sets_full_and_is_off_while_the_width_forces_the_rail(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        // The user chose the rail: expand sets 300, it does not flip.
        this.actions.board_hidden = true;
        assert!(this.expand_sidebar());
        assert!(!this.actions.board_hidden);
        assert!(this.expand_sidebar());
        assert!(!this.actions.board_hidden, "a second tap stays at 300");
        // The window forces the rail (the user wants 300, the frame is
        // collapsed): the button is off and nothing is saved.
        this.shell.frame.sidebar_collapsed = true;
        assert!(this.sidebar_forced_rail());
        assert_eq!(
            expand_unavailable(this.sidebar_user_mode(), true),
            Some("Window too narrow")
        );
        assert!(!this.expand_sidebar());
        this.toggle_sidebar();
        assert!(
            !this.actions.board_hidden,
            "Command-B cannot change what the width forces"
        );
        // With the rail chosen by the user, the width is not the reason.
        this.actions.board_hidden = true;
        assert!(!this.sidebar_forced_rail());
        assert_eq!(expand_unavailable(this.sidebar_user_mode(), true), None);
        assert!(this.expand_sidebar());
        assert!(!this.actions.board_hidden);
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
