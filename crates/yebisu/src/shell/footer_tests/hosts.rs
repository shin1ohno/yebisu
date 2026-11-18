use super::*;

/// Plan v2 7.6: footer Hosts goes where the toolbar Hosts went. Since L7 that
/// is the Hosts sheet; when `open_hosts` declines it is still the Keys and
/// hosts screen (the terminal closes, as the toolbar button did).
#[gpui::test]
fn footer_hosts_returns_to_keys_and_hosts(cx: &mut TestAppContext) {
    assert_eq!(hosts_target(true), HostsTarget::Sheet);
    assert_eq!(hosts_target(false), HostsTarget::KeysAndHosts);
    let (view, cx) = window(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.open = true;
            this.footer_tap(FooterItem::Hosts, window, cx);
            assert!(
                this.shell.hosts_sheet.list_open,
                "Hosts opens the host list"
            );
            assert!(this.hosts_sheet_modal());
            assert!(this.terminal.open, "the sheet opens over the terminal");
            this.hosts_close(cx);
            // The declined path: back to the Keys and hosts screen.
            let selection = this.board.selection;
            this.return_to_keys_and_hosts(window, cx);
            assert!(!this.terminal.open);
            assert!(!this.smoke_mode);
            assert_eq!(this.board.selection, selection.wrapping_add(1));
        });
    });
}
