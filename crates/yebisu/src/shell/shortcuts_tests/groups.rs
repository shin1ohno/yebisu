//! The canvas' TABS / PANES / APP groups and the footer line.
use super::super::view::settings_line;
use super::super::*;
use super::support::{connected, terminal};
use sheprd_commands::Command;

#[test]
fn rows_are_grouped_like_the_canvas() {
    let rows = popover_rows(&terminal(&["tab.create"]), &connected());
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

#[test]
fn footer_shows_command_key_and_priority() {
    assert_eq!(
        settings_line(false, false),
        "Command key: Local · Shortcut priority: App"
    );
    assert_eq!(
        settings_line(true, true),
        "Command key: Ctrl · Shortcut priority: Terminal"
    );
}

#[test]
fn shortcuts_hint_only_with_a_hardware_keyboard() {
    use crate::shell::shortcuts::{show_hold_hint, view::HINT};
    assert_eq!(show_hold_hint(false), None);
    assert_eq!(show_hold_hint(true), Some(HINT));
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
        let collapsed = canvas_rows(group, rows.clone(), false);
        let expanded = canvas_rows(group, rows, true);
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
                    ]
                );
            }
        }
    }
}
