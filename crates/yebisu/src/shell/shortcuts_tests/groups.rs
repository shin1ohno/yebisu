//! The canvas' TABS / PANES / APP groups and the footer line.
use super::super::*;
use super::support::{connected, terminal};
use gpui::{AppContext as _, TestAppContext};
use sheprd_commands::Command;

#[test]
fn rows_are_grouped_like_the_canvas() {
    let rows = popover_rows(
        &terminal(&["tab.create"]),
        &connected(),
        &Actions::default(),
    );
    let total = rows.len();
    let groups = grouped(rows);
    let titles: Vec<_> = groups.iter().map(|(group, _)| group.title()).collect();
    assert_eq!(titles, ["TABS", "PANES", "APP"]);
    // Grouping neither drops nor duplicates a catalog row.
    assert_eq!(
        groups.iter().map(|(_, rows)| rows.len()).sum::<usize>(),
        total
    );
    let group_of = |command: Command| {
        groups
            .iter()
            .find(|(_, rows)| rows.iter().any(|(c, _)| *c == command))
            .map(|(group, _)| *group)
    };
    assert_eq!(group_of(Command::Tab), Some(ShortcutGroup::Tabs));
    assert_eq!(group_of(Command::SplitRight), Some(ShortcutGroup::Panes));
    assert_eq!(group_of(Command::ToggleSidebar), Some(ShortcutGroup::App));
}

#[gpui::test]
fn footer_shows_settings_for_each_shortcut_style(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.actions.herdr_prefix = false;
        this.terminal.input.options.command_control = false;
        this.actions.terminal_priority = false;
        assert_eq!(
            this.shortcuts_settings_line(),
            "Command key: Local · Shortcut priority: App"
        );
        this.terminal.input.options.command_control = true;
        this.actions.terminal_priority = true;
        assert_eq!(
            this.shortcuts_settings_line(),
            "Command key: Ctrl · Shortcut priority: Terminal"
        );

        this.actions.herdr_prefix = true;
        this.terminal.reconnect.target = "user@pro-dev.home.local".into();
        assert_eq!(
            this.shortcuts_settings_line(),
            "Shortcut style: Herdr prefix · keys from pro-dev's Herdr config"
        );
    });
}

#[test]
fn shortcuts_show_the_canvas_focus_and_app_groups() {
    use crate::shell::shortcuts::{PopoverAction, canvas_rows};
    let state = ChromeState {
        connected: true,
        ..ChromeState::default()
    };
    let rows = rows_from(actions::catalog(&TerminalView::default()), &state, false);
    for (group, rows) in grouped(rows) {
        let collapsed = canvas_rows(group, rows.clone(), false, false);
        let expanded = canvas_rows(group, rows, true, false);
        let labels: Vec<_> = collapsed
            .iter()
            .map(|(_, row)| row.label.as_str())
            .collect();
        let order: Vec<_> = collapsed.iter().map(|(action, _)| action.clone()).collect();
        match group {
            ShortcutGroup::Tabs => {
                assert_eq!(
                    order,
                    [
                        PopoverAction::Catalog(Command::Tab),
                        PopoverAction::Catalog(Command::CloseTab),
                        PopoverAction::Catalog(Command::NextTab),
                        PopoverAction::Catalog(Command::PreviousTab),
                        PopoverAction::Focus(ShortcutGroup::Tabs),
                    ]
                );
                assert_eq!(
                    labels
                        .iter()
                        .filter(|label| label.starts_with("Focus Tab"))
                        .copied()
                        .collect::<Vec<_>>(),
                    ["Focus Tab 1–9"]
                );
                assert_eq!(expanded.len() - collapsed.len(), 9);
            }
            ShortcutGroup::Panes => {
                assert_eq!(
                    order,
                    [
                        PopoverAction::Catalog(Command::SplitRight),
                        PopoverAction::Catalog(Command::SplitDown),
                        PopoverAction::Catalog(Command::ClosePane),
                        PopoverAction::Catalog(Command::NextPane),
                        PopoverAction::Catalog(Command::PreviousPane),
                        PopoverAction::Focus(ShortcutGroup::Panes),
                        PopoverAction::Catalog(Command::Zoom),
                    ]
                );
                assert!(labels.contains(&"Focus Left · Right · Up · Down"));
                assert_eq!(expanded.len() - collapsed.len(), 4);
            }
            ShortcutGroup::App => {
                assert_eq!(
                    order,
                    [
                        PopoverAction::Catalog(Command::ToggleSidebar),
                        PopoverAction::Catalog(Command::Palette),
                        PopoverAction::Settings,
                    ]
                );
            }
        }
    }
}

#[test]
fn hint_popover_shows_help_settings_and_ordered_focus_keys() {
    let mut terminal = terminal(&["tab.create"]);
    terminal.reconnect.target = "sandbox".into();
    let mut actions = Actions::default();
    assert_eq!(
        header_hint(&terminal, &actions),
        "Tap a row to run it. Open with ⌘/."
    );
    actions.herdr_prefix = true;
    actions.host_keys.set_test_config(
        &terminal,
        "[keys]\nprefix = 'ctrl+a'\nnew_tab = ['prefix+c', 'alt+t']\nhelp = 'prefix+F1'",
    );
    assert_eq!(
        header_hint(&terminal, &actions),
        "Tap a row to run it. Open with ctrl+a F1."
    );
    let rows = popover_rows(&terminal, &connected(), &actions);
    assert!(
        rows.iter()
            .any(|(cmd, row)| *cmd == Command::Tab && row.keys == "ctrl+a c / alt+t")
    );
    assert!(
        rows.iter()
            .any(|(cmd, row)| *cmd == Command::Palette && row.keys == "—")
    );
    for (group, members) in grouped(rows) {
        let rows = canvas_rows(group, members, true, true);
        match group {
            ShortcutGroup::Tabs => {
                assert!(
                    rows.iter()
                        .any(|(_, row)| row.label == "Focus Tab 1–9" && row.keys == "ctrl+a 1…9")
                );
            }
            ShortcutGroup::Panes => {
                assert!(
                    rows.iter().any(
                        |(_, row)| row.label == "Focus ← → ↑ ↓" && row.keys == "ctrl+a h l k j"
                    )
                );
                let directions: Vec<_> = rows.iter().filter_map(|(action, row)|
                    matches!(action, PopoverAction::Catalog(command) if is_grouped_focus(*command))
                        .then_some(row.keys.as_str())).collect();
                assert_eq!(directions, ["ctrl+a h", "ctrl+a l", "ctrl+a k", "ctrl+a j"]);
            }
            ShortcutGroup::App => {
                for herdr in [false, true] {
                    let app = canvas_rows(group, Vec::new(), false, herdr);
                    assert!(app.iter().any(
                        |(action, row)| *action == PopoverAction::Settings && row.keys == "⌘,"
                    ));
                }
            }
        }
    }
    terminal.live.connected = false;
    assert_eq!(header_hint(&terminal, &actions), "Tap a row to run it.");
    assert!(
        popover_rows(&terminal, &ChromeState::default(), &actions)
            .iter()
            .all(|(_, row)| row.keys == "—")
    );
}
