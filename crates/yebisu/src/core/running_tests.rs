//! `running`: which Herdr 2 terminals make Close Space ask first (#231).
use super::{is_shell, runs_program};
use serde_json::{Value, json};
use sheprd_core_client::wire::Terminal;

type Outcome = Result<(), Box<dyn std::error::Error>>;

fn terminal(fields: Value) -> Result<Terminal, serde_json::Error> {
    let mut value = json!({"address": "t1", "state": "live"});
    if let (Some(base), Some(extra)) = (value.as_object_mut(), fields.as_object()) {
        for (key, field) in extra {
            base.insert(key.clone(), field.clone());
        }
    }
    serde_json::from_value(value)
}

#[test]
fn shells_are_named_as_the_official_app_reads_them() {
    for name in [
        "zsh",
        "-zsh",
        "/bin/bash",
        "fish",
        "pwsh.exe",
        "C:\\x\\cmd.EXE",
    ] {
        assert!(is_shell(name), "{name}");
    }
    for name in ["claude", "vim", "node", "zshx", "", "python3"] {
        assert!(!is_shell(name), "{name}");
    }
}

#[test]
fn a_shell_at_its_prompt_runs_nothing() -> Outcome {
    assert!(!runs_program(&terminal(json!({"argv": ["sh"]}))?));
    let idle = json!({"argv": ["/bin/zsh", "-l"],
        "foreground": {"pid": 7, "name": "zsh", "argv": ["-zsh"]}});
    assert!(!runs_program(&terminal(idle)?));
    // No program named at all: herdr-core started its default shell.
    assert!(!runs_program(&terminal(json!({}))?));
    Ok(())
}

#[test]
fn a_program_in_the_foreground_runs_something() -> Outcome {
    let vim =
        json!({"argv": ["zsh"], "foreground": {"pid": 9, "name": "vim", "argv": ["vim", "a.rs"]}});
    assert!(runs_program(&terminal(vim)?));
    // The foreground names no argv: its process name is read.
    let named = json!({"argv": ["zsh"], "foreground": {"pid": 9, "name": "top"}});
    assert!(runs_program(&terminal(named)?));
    // Started as the program itself, with no foreground reported.
    assert!(runs_program(&terminal(json!({"argv": ["claude"]}))?));
    Ok(())
}

#[test]
fn a_reported_status_runs_something_even_in_a_shell() -> Outcome {
    let agent = json!({"argv": ["sh"], "status": {"state": "idle", "app": "claude"}});
    assert!(runs_program(&terminal(agent)?));
    Ok(())
}

#[test]
fn an_ended_terminal_runs_nothing() -> Outcome {
    let ended = json!({"state": "finished", "reason": "exited", "argv": ["claude"],
        "status": {"state": "done"}});
    assert!(!runs_program(&terminal(ended)?));
    Ok(())
}
