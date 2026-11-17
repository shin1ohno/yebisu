use super::*;

#[gpui::test]
fn first_run_opens_add_host_sheet_over_empty_frame(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Before the saved hosts load, the frame is drawn without a sheet:
            // a returning user must not see Add host flash up.
            assert!(this.render_first_run(window, cx).is_some());
            assert!(this.render_hosts_sheet(window, cx).is_none());
            // Zero hosts after the load: the frame plus the Add host sheet.
            this.setup.loaded = true;
            assert!(this.setup.hosts.is_empty());
            assert!(this.render_first_run(window, cx).is_some());
            let session = this.shell.hosts_sheet.add.as_ref();
            assert_eq!(session.map(|s| s.mode), Some(SheetMode::Add));
            assert!(this.render_hosts_sheet(window, cx).is_some());
            // The auto-open happens once: Cancel leaves the empty frame.
            this.hosts_close(cx);
            assert!(this.render_first_run(window, cx).is_some());
            assert!(this.render_hosts_sheet(window, cx).is_none());
            // With a terminal open there is no first-run frame.
            this.terminal.open = true;
            assert!(this.render_first_run(window, cx).is_none());
        });
    });
}

#[test]
fn saved_hosts_open_the_list_instead_of_add_host() {
    let mut state = HostsSheetState::default();
    assert!(!state.auto_open(false, 0), "waits for the load");
    assert!(state.add.is_none() && !state.list_open);
    assert!(state.auto_open(true, 2));
    assert!(state.list_open && state.add.is_none());
    assert!(!state.auto_open(true, 2), "only once");
}

#[test]
fn zero_hosts_open_add_host_once() {
    let mut state = HostsSheetState::default();
    assert!(state.auto_open(true, 0));
    assert_eq!(state.add.as_ref().map(|s| s.mode), Some(SheetMode::Add));
    state.close();
    assert!(!state.auto_open(true, 0));
    assert!(!state.visible());
}
