use super::*;

#[test]
fn hint_settings_lists_full_host_labels_and_local_keys_in_both_styles() {
    let mut terminal = TerminalView::default();
    terminal.reconnect.target = "sandbox".into();
    terminal.live.connected = true;
    let mut actions = Actions::default();
    actions.host_keys.set_test_config(
        &terminal,
        "[keys]\nprefix = 'ctrl+a'\nnew_tab = ['prefix+c', 'alt+t']",
    );
    for herdr in [false, true] {
        actions.herdr_prefix = herdr;
        let rows = shortcut_rows(&terminal, &actions);
        let display = |label: &str| {
            rows.iter()
                .find(|row| row.label == label)
                .unwrap_or_else(|| panic!("missing {label}"))
                .display(herdr)
        };
        let new_tab = display("New Tab");
        if herdr {
            assert!(
                new_tab.starts_with("New Tab · ctrl+a c / alt+t"),
                "{new_tab}"
            );
        } else {
            let defaults = sheprd_commands::COMMANDS
                .iter()
                .find(|info| info.command == Command::Tab)
                .map_or(&[][..], |info| info.default_shortcuts())
                .join(", ");
            assert!(!defaults.is_empty());
            assert!(
                new_tab.starts_with(&format!("New Tab · {defaults}")),
                "{new_tab}"
            );
        }
        assert_eq!(display("Settings"), "Settings · ⌘,");
        assert_eq!(display("Copy"), "Copy · ⌘C");
        assert_eq!(display("Paste"), "Paste · ⌘V");
        assert!(display("Keyboard shortcuts").contains(if herdr { "ctrl+a ?" } else { "⌘/" }));
        if herdr {
            assert!(display("Command Palette").starts_with("Command Palette · —"));
        }
    }
    terminal.live.connected = false;
    assert!(
        shortcut_rows(&terminal, &actions)
            .iter()
            .any(|row| row.label == "New Tab" && row.display(true) == "New Tab · — (unavailable)")
    );
}
