//! Automatic space names (issue #144, owner decision 9): like the official
//! app, an unnamed space is called after its folder's git checkout top,
//! found on the host over ssh, the way `DISCOVER` reads the host.
//!
//! The folders come from the document, so they are untrusted text: the
//! command carries them hex-encoded inside a quoted here-document and the
//! script decodes them as data (`names.py`); nothing of them reaches the
//! shell. Until a name arrives, the row shows the folder's last component.
use std::collections::BTreeMap;

/// The longest answer read, like discovery's bound.
#[cfg(all(target_os = "ios", not(test)))]
const MAX_OUTPUT: usize = 16_384;

/// The ssh command that names `folders`.
pub(crate) fn command(folders: &[String]) -> String {
    let joined = folders.join("\0");
    let hex = joined
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "if command -v python3 >/dev/null 2>&1; then\n    exec python3 -I - <<'YEBISU_CORE_NAMES'\nFOLDERS = \"{hex}\"\n{}\nYEBISU_CORE_NAMES\nfi\nexit 0\n",
        include_str!("names.py"),
    )
}

/// Each folder's display name from the script's answer: the last path
/// component of its checkout top, or `~` for the home folder (owner answer
/// Q2). An answer cut short keeps only its whole pairs.
pub(crate) fn parse(output: &[u8]) -> BTreeMap<String, String> {
    let mut fields = output.split(|byte| *byte == 0);
    let Some(home) = fields.next().map(String::from_utf8_lossy) else {
        return BTreeMap::new();
    };
    let mut names = BTreeMap::new();
    let mut rest = fields.collect::<Vec<_>>();
    // The split leaves an empty field after the last NUL.
    if rest.last().is_some_and(|last| last.is_empty()) {
        rest.pop();
    }
    for pair in rest.chunks_exact(2) {
        let folder = String::from_utf8_lossy(pair[0]).into_owned();
        let top = String::from_utf8_lossy(pair[1]);
        let name = if !home.is_empty() && top == home {
            "~".to_owned()
        } else {
            last_component(&top)
        };
        if !folder.is_empty() && !name.is_empty() {
            names.insert(folder, name);
        }
    }
    names
}

/// The name a row shows for an unnamed space in `folder`.
pub(crate) fn display(folder: &str, names: &BTreeMap<String, String>) -> String {
    let name = names
        .get(folder)
        .cloned()
        .unwrap_or_else(|| last_component(folder));
    super::session::one_line(&name)
}

fn last_component(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Asks the host; `None` when ssh or the script failed.
#[cfg(all(target_os = "ios", not(test)))]
pub(crate) fn ask(
    host: &str,
    folders: &[String],
    cancelled: impl Fn() -> bool,
) -> Option<BTreeMap<String, String>> {
    let (exit, output) = sheprd_client::run_ssh_command(
        host,
        &command(folders),
        std::time::Duration::from_secs(3),
        MAX_OUTPUT,
        cancelled,
    )
    .ok()?;
    exit.success().then(|| parse(&output))
}
