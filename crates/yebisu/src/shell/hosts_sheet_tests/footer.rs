use super::*;

#[gpui::test]
fn hosts_footer_opens_host_list(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Footer Hosts calls open_hosts; it must report the tap handled.
            assert!(this.shell.hosts_sheet.add.is_none() && !this.shell.hosts_sheet.list_open);
            assert!(
                this.open_hosts(cx),
                "open_hosts must report the tap handled"
            );
            assert!(this.shell.hosts_sheet.list_open);
            assert!(this.shell.hosts_sheet.add.is_none());
            assert!(this.render_hosts_sheet(window, cx).is_some());
            // Open over a connected terminal: the same sheet, input fenced.
            this.shell.hosts_sheet.close();
            this.terminal.open = true;
            assert!(this.open_hosts(cx));
            assert!(this.hosts_sheet_modal());
            assert!(this.render_hosts_sheet(window, cx).is_some());
            this.hosts_close(cx);
            assert!(this.render_hosts_sheet(window, cx).is_none());
            assert!(!this.hosts_sheet_modal());
        });
    });
}
