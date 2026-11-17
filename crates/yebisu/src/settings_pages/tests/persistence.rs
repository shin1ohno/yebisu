//! Q7, q7: the Input options and Return are written and read back by a
//! fresh process; files from older builds and unknown names load defaults.
use super::support::{directory, open, wait, write_file};
use crate::settings::{SettingsHost, Values};
use crate::settings_pages::choices::{Choice, INPUT};

/// The three Input options that were not stored before L1 (Q7).
const NEW_INPUT: [Choice; 3] = [
    Choice::OptionKey,
    Choice::CommandKey,
    Choice::JapaneseCandidates,
];

#[test]
fn input_options_survive_restart() {
    for choice in NEW_INPUT {
        let root = directory(choice.id());
        let mut host = open(&root);
        assert!(
            !choice.get(&host.draft),
            "{choice:?} defaults to its first label"
        );
        let mut values = host.draft.clone();
        choice.set(&mut values, true);
        host.edit(values);
        wait(&mut host, SettingsHost::ready);
        drop(host);
        let restarted = open(&root);
        assert_eq!(restarted.error, None);
        for row in INPUT {
            assert_eq!(
                row.get(&restarted.draft),
                row == choice || row.get(&Values::default()),
                "{choice:?} saved; {row:?} after restart"
            );
        }
    }
}

#[test]
fn return_policy_survives_restart() {
    let root = directory("return");
    let mut host = open(&root);
    assert!(
        host.draft.reconnect_automatic,
        "Return defaults to Automatic"
    );
    let mut values = host.draft.clone();
    Choice::Return.set(&mut values, false);
    host.edit(values);
    wait(&mut host, SettingsHost::ready);
    drop(host);
    let mut restarted = open(&root);
    assert!(!restarted.draft.reconnect_automatic, "Tap survives restart");
    // Back to the default: the next process reads Automatic again.
    let mut values = restarted.draft.clone();
    Choice::Return.set(&mut values, true);
    restarted.edit(values);
    wait(&mut restarted, SettingsHost::ready);
    drop(restarted);
    assert!(open(&root).draft.reconnect_automatic);
}

#[test]
fn old_store_without_input_options_loads_defaults() {
    // The five stored fields of the file written before L1.
    let root = directory("old-store");
    write_file(
        &root,
        &serde_json::json!({"version":1,"language":"en-US","compact":true,
            "terminal_priority":true,"board_hidden":false,"drag_select":false,
            "copy_on_select":true}),
    );
    let host = open(&root);
    assert_eq!(host.error, None, "an old file still loads");
    assert!(host.draft.compact && host.draft.terminal_priority);
    assert!(!host.draft.drag_select && host.draft.copy_on_select);
    let defaults = Values::default();
    for choice in NEW_INPUT.into_iter().chain([Choice::Return]) {
        assert_eq!(
            choice.get(&host.draft),
            choice.get(&defaults),
            "{choice:?} loads its default"
        );
    }
}

#[test]
fn unknown_input_value_falls_back_to_default() {
    let root = directory("unknown");
    write_file(
        &root,
        &serde_json::json!({"version":1,"language":"en-US","compact":false,
            "terminal_priority":false,"board_hidden":false,
            "option_key":"meta","command_key":7,"japanese_candidates":null,
            "return_policy":"sometimes"}),
    );
    let host = open(&root);
    assert_eq!(host.error, None, "an unknown name is not a load error");
    let defaults = Values::default();
    for choice in NEW_INPUT.into_iter().chain([Choice::Return]) {
        assert_eq!(choice.get(&host.draft), choice.get(&defaults), "{choice:?}");
    }
    // Known names still load next to the unknown ones.
    let root = directory("known");
    write_file(
        &root,
        &serde_json::json!({"version":1,"language":"en-US","compact":false,
            "terminal_priority":false,"board_hidden":false,
            "option_key":"alt","command_key":"ctrl","japanese_candidates":"caret",
            "return_policy":"tap"}),
    );
    let host = open(&root);
    assert!(host.draft.option_alt && host.draft.command_control && host.draft.caret_anchor);
    assert!(!host.draft.reconnect_automatic);
}
