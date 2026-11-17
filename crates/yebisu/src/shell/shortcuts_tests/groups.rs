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
        "Command key: Control · Shortcut priority: Terminal"
    );
}
