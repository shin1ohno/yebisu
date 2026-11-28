//! `named_close`: what a Close Space of the connected `space-<ms>` session
//! ends, and when it refuses (#249).
use super::{
    JUST_STARTED, Listed, NOT_ANSWERING, PROTECTED, STARTED_SINCE, judged_pid, listed_closable,
    observe_listed, plan,
};
use crate::core::vacate::GRACE;
use serde_json::{Value, json};
use sheprd_core_client::wire::Terminal;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

type Outcome = Result<(), Box<dyn std::error::Error>>;

fn terminal(address: &str, state: &str, argv: &[&str]) -> Result<Terminal, serde_json::Error> {
    let value: Value = json!({"address": address, "state": state, "argv": argv});
    serde_json::from_value(value)
}

fn refused(result: crate::Result<Vec<String>>) -> String {
    match result {
        Ok(ending) => panic!("closed, ending {ending:?}"),
        Err(error) => error.to_string(),
    }
}

/// Shells alone: every live terminal ends, whether a tab of this iPad holds
/// it or not; a finished one is left as it is.
#[test]
fn shells_alone_end_every_live_terminal() -> Outcome {
    let terminals = [
        terminal("t1", "live", &["zsh"])?,
        terminal("t2", "live", &["sh"])?,
        terminal("t3", "finished", &["vim"])?,
    ];
    assert_eq!(plan("space-17", &[], &terminals)?, ["t1", "t2"]);
    Ok(())
}

/// The busy terminals the question named end with the shells.
#[test]
fn the_asked_programs_end() -> Outcome {
    let terminals = [
        terminal("t1", "live", &["zsh"])?,
        terminal("t2", "live", &["sleep", "600"])?,
    ];
    assert_eq!(plan("space-17", &["t2".into()], &terminals)?, ["t1", "t2"]);
    Ok(())
}

/// A program the user was not asked about (it started after the question,
/// or the poll had not seen it) refuses the close: nothing ends.
#[test]
fn a_program_not_asked_about_refuses() -> Outcome {
    let terminals = [
        terminal("t1", "live", &["sleep", "600"])?,
        terminal("t2", "live", &["claude"])?,
    ];
    assert!(refused(plan("space-17", &[], &terminals)).contains(STARTED_SINCE));
    assert!(refused(plan("space-17", &["t1".into()], &terminals)).contains(STARTED_SINCE));
    Ok(())
}

/// `default` is never closed this way, even holding only shells.
#[test]
fn default_is_refused() -> Outcome {
    let terminals = [terminal("t1", "live", &["zsh"])?];
    assert!(refused(plan("default", &[], &terminals)).contains(PROTECTED));
    Ok(())
}

fn with_foreground(address: &str, argv: &[&str], fg: &str) -> Result<Terminal, serde_json::Error> {
    serde_json::from_value(json!({"address": address, "state": "live", "argv": argv,
        "foreground": {"pid": 9, "name": fg, "argv": [fg]}}))
}

/// #249 B: the poll's record of a listed space: the live terminals a Close
/// ends, the busy ones it asks about (a shell's foreground program counts),
/// and since when this same server has been seen.
#[test]
fn a_listed_space_keeps_its_server_and_terminals() -> Outcome {
    let start = Instant::now();
    let later = start + Duration::from_secs(7);
    let terminals = vec![
        with_foreground("t1", &["sh"], "sleep")?,
        terminal("t2", "live", &["zsh"])?,
        terminal("t3", "finished", &["vim"])?,
    ];
    let mut listed = BTreeMap::new();
    let seen = |pid: u64| {
        vec![
            (
                "space-17".to_owned(),
                "core-a".to_owned(),
                Some(pid),
                terminals.clone(),
            ),
            (
                "default".to_owned(),
                "core-d".to_owned(),
                Some(1),
                terminals.clone(),
            ),
        ]
    };
    observe_listed(&mut listed, start, seen(40));
    let row = listed
        .get("space-17")
        .cloned()
        .unwrap_or_else(|| panic!("not listed"));
    assert_eq!(row.expect.core_id, "core-a");
    assert_eq!(row.expect.pid, Some(40));
    assert_eq!(
        row.expect.end,
        [("t1".to_owned(), Some(9)), ("t2".to_owned(), None)]
    );
    assert_eq!(row.busy, ["t1"]);
    assert!(!listed.contains_key("default"), "default never closes");
    // The same server keeps the time it was first seen.
    observe_listed(&mut listed, later, seen(40));
    assert_eq!(listed.get("space-17").map(|row| row.since), Some(start));
    // Another server (a restart: another pid) starts again.
    observe_listed(&mut listed, later, seen(41));
    assert_eq!(listed.get("space-17").map(|row| row.since), Some(later));
    // A poll that does not find it answering forgets it.
    observe_listed(&mut listed, later, Vec::new());
    assert!(listed.is_empty());
    Ok(())
}

/// A listed space closes only once the poll has seen the same server for
/// `GRACE`: another client's space, just created, is left alone.
#[test]
fn a_listed_space_closes_only_after_grace() {
    let since = Instant::now();
    let row = Listed {
        expect: crate::core::vacate::Expect::seen("core-a".into(), Some(40)),
        busy: Vec::new(),
        since,
    };
    assert_eq!(listed_closable(None, since), Err(NOT_ANSWERING));
    assert_eq!(listed_closable(Some(&row), since), Err(JUST_STARTED));
    assert_eq!(
        listed_closable(Some(&row), since + GRACE - Duration::from_millis(1)),
        Err(JUST_STARTED)
    );
    assert_eq!(listed_closable(Some(&row), since + GRACE), Ok(()));
}

/// Review of #255: the stop compares each seen terminal's judged program by
/// pid, its foreground else its own program, so a program started in a seen
/// shell is a change, and a foreground reported only later is not.
#[test]
fn a_terminal_is_judged_by_its_foreground_else_its_own_program() -> Outcome {
    let both: Terminal = serde_json::from_value(json!({"address": "t1", "state": "live",
        "pid": 5, "foreground": {"pid": 9, "name": "vim"}}))?;
    assert_eq!(judged_pid(&both), Some(9));
    let own: Terminal =
        serde_json::from_value(json!({"address": "t1", "state": "live", "pid": 5}))?;
    assert_eq!(judged_pid(&own), Some(5));
    assert_eq!(judged_pid(&terminal("t1", "live", &["sleep"])?), None);
    Ok(())
}
