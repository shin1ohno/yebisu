//! Automatic space names (#144 decision 9, owner answer Q2): the command
//! carries document folders only as hex data, the answer parses by NUL,
//! and a row shows the checkout's last component or `~` for home.
use crate::shell::herdr2_frame::names::{command, display, parse};
use std::collections::BTreeMap;

#[test]
fn folders_reach_the_script_only_as_hex() {
    let hostile = "/tmp/it's $(touch /tmp/x)\n`id`; rm -rf ~";
    let script = command(&["/Users/owner/src/alpha".into(), hostile.into()]);
    let hex = format!("{}{}{}", hex("/Users/owner/src/alpha"), "00", hex(hostile));
    assert!(script.contains(&format!("FOLDERS = \"{hex}\"")), "{script}");
    // Nothing of the folders is in the text the shell reads.
    for piece in ["it's", "$(touch", "`id`", "rm -rf", "/Users/owner"] {
        assert!(!script.contains(piece), "{piece} reached the command");
    }
    // A quoted here-document: the shell expands nothing inside it.
    assert!(script.contains("<<'YEBISU_CORE_NAMES'"));
    assert!(script.contains("python3 -I -"));
}

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn the_answer_parses_by_nul() {
    let answer = b"/Users/owner\0/Users/owner/src/alpha/sub\0/Users/owner/src/alpha\0/Users/owner\0/Users/owner\0/tmp/x\0/tmp/x\0";
    let names = parse(answer);
    assert_eq!(
        names,
        BTreeMap::from([
            ("/Users/owner".to_owned(), "~".to_owned()),
            ("/Users/owner/src/alpha/sub".to_owned(), "alpha".to_owned()),
            ("/tmp/x".to_owned(), "x".to_owned()),
        ])
    );
    // A cut answer keeps whole pairs only; an empty one names nothing.
    assert_eq!(parse(b"/home/me\0/a\0/a/top\0/b\0").len(), 1);
    assert!(parse(b"").is_empty());
}

#[test]
fn rows_show_the_last_component_until_named() {
    let names = BTreeMap::from([("/Users/owner/src/alpha/sub".to_owned(), "alpha".to_owned())]);
    assert_eq!(display("/Users/owner/src/alpha/sub", &names), "alpha");
    assert_eq!(display("/Users/owner/src/beta/", &names), "beta");
    assert_eq!(display("", &names), "");
    // Document text is one line on a row.
    assert_eq!(display("/tmp/a\nb", &BTreeMap::new()), "a b");
}

#[test]
fn the_script_is_valid_python() {
    // `names.py` compiles with the folders line yebisu puts above it.
    let script = command(&["/tmp".into()]);
    let body = script
        .split("<<'YEBISU_CORE_NAMES'\n")
        .nth(1)
        .and_then(|rest| rest.split("\nYEBISU_CORE_NAMES\n").next())
        .unwrap_or_else(|| panic!("no here-document in {script}"));
    let checked = std::process::Command::new("python3")
        .args([
            "-I",
            "-c",
            "import sys; compile(sys.stdin.read(), 'names.py', 'exec')",
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(body.as_bytes())?;
            }
            child.wait()
        })
        .unwrap_or_else(|e| panic!("python3: {e}"));
    assert!(checked.success());
}
