//! `named_close`: what a Close Space of the connected `space-<ms>` session
//! ends, and when it refuses (#249).
use super::{PROTECTED, STARTED_SINCE, plan};
use serde_json::{Value, json};
use sheprd_core_client::wire::Terminal;

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
