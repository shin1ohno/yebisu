//! A Herdr 2 space whose last tab is gone goes away with its server.
//!
//! Each space is its own herdr-core server (`hc connect --session
//! space-<ms>` starts one), so a space that holds no tab and no terminal
//! that is not finished is only an empty row in the sidebar. The owner's rule
//! (2026-11-24): stop that space's server so the space disappears from every
//! client. The history of its finished terminals goes with the server.
//!
//! Rules kept here, applied by `worker.rs` (the connected space) and by the
//! discovery poll in `core.rs` (the other spaces of the host):
//! - the space named `default` is never stopped;
//! - a space with a terminal whose state is not finished, even one in no tab,
//!   is never stopped: the list is taken just before stopping;
//! - a space found vacant by a poll rather than by its own last tab closing is
//!   stopped only once two snapshots at least `GRACE` apart both found it
//!   vacant, so another client's space, just created, is left alone;
//! - the server is stopped only if it is still the one that was observed.
use super::SessionInfo;
use sheprd_core_client::wire::{Terminal, TerminalState};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

/// How long a space found vacant by a poll must stay so before it is stopped.
pub(super) const GRACE: Duration = Duration::from_secs(5);

/// The space of installs that picked no named space: never stopped.
pub(super) fn protected(name: &str) -> bool {
    name == crate::shell::herdr2_frame::HERDR2_SESSION
}

/// Whether a space with `tabs` tabs and the terminals `terminals` (a fresh
/// `terminal.list`) holds nothing: no tab and no terminal whose state is not
/// finished. A terminal that is live, or in a state this client does not know
/// (`Other`), keeps the space.
pub(super) fn vacant(tabs: usize, terminals: &[Terminal]) -> bool {
    tabs == 0
        && terminals
            .iter()
            .all(|terminal| terminal.state == TerminalState::Finished)
}

/// Stops the server of `session` if its core id is still the one given.
pub(crate) type Stopper = Arc<dyn Fn(&str, &str) -> crate::Result<()> + Send + Sync>;

/// The stopper for the host's spaces: SSH to the host on the iPad.
pub(crate) fn stopper(host: &str) -> Stopper {
    #[cfg(test)]
    if let Some(stopper) = registry::get(host) {
        return stopper;
    }
    ssh_stopper(host)
}

#[cfg(test)]
pub(crate) use registry::set as set_test_stopper;

#[cfg(test)]
mod registry {
    use super::Stopper;
    use std::{collections::BTreeMap, sync::Mutex};

    static STOPPERS: Mutex<BTreeMap<String, Stopper>> = Mutex::new(BTreeMap::new());

    /// Tests name the host they connect to; its spaces stop through `stopper`.
    pub(crate) fn set(host: &str, stopper: Stopper) {
        STOPPERS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(host.to_owned(), stopper);
    }

    pub(super) fn get(host: &str) -> Option<Stopper> {
        STOPPERS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(host)
            .cloned()
    }
}

#[cfg(target_os = "ios")]
fn ssh_stopper(host: &str) -> Stopper {
    let host = host.to_owned();
    Arc::new(move |session, core_id| {
        let command = stop_command(session, core_id)?;
        let (exit, output) =
            sheprd_client::run_ssh_command(&host, &command, Duration::from_secs(10), 4096, || {
                false
            })
            .map_err(|error| crate::Error::Ssh(error.to_string()))?;
        stop_result(exit.code(), &output)
    })
}

/// What the exit status and output of `stop_command` mean.
#[cfg_attr(not(any(target_os = "ios", test)), allow(dead_code))]
pub(crate) fn stop_result(code: Option<i32>, output: &[u8]) -> crate::Result<()> {
    match code {
        Some(0) => Ok(()),
        Some(STOPPED_ELSEWHERE) => Err(crate::Error::Ssh(
            "Herdr 2 space is not the server that was seen; left running".into(),
        )),
        Some(127) => Err(crate::Error::Ssh("Herdr 2 is not installed".into())),
        _ => Err(crate::Error::Ssh(format!(
            "Herdr 2 server did not stop: {}",
            crate::shell::herdr2_frame::session::one_line(&String::from_utf8_lossy(output))
        ))),
    }
}

#[cfg(not(target_os = "ios"))]
fn ssh_stopper(_host: &str) -> Stopper {
    Arc::new(|_, _| {
        Err(crate::Error::Ssh(
            "SSH unavailable on this host: cannot stop a Herdr 2 space".into(),
        ))
    })
}

/// The exit status of `stop_command` when the server answering is not the one
/// that was observed (restarted, or another server behind the same name).
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
const STOPPED_ELSEWHERE: i32 = 3;

/// The remote POSIX command that stops one space's server. It looks for `hc`
/// where `core_connect_command` does, and only stops the server whose
/// `hc server status --json` names `core_id`. `hc server stop` is the CLI of
/// the `server.stop` request.
#[cfg_attr(not(any(target_os = "ios", test)), allow(dead_code))]
pub(crate) fn stop_command(session: &str, core_id: &str) -> crate::Result<String> {
    // Validates the session name the way the connect command does.
    sheprd_client::core_connect_command(session).map_err(|e| crate::Error::Ssh(e.to_string()))?;
    if core_id.is_empty()
        || !core_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(crate::Error::Ssh("Herdr 2 core id is unusable".into()));
    }
    let session = word(session);
    let script = format!(
        r#"for path in "$(command -v hc 2>/dev/null || true)" "$(command -v herdr-core 2>/dev/null || true)" "$HOME/.local/bin/hc" "$HOME/.local/bin/herdr-core" "$HOME/.cargo/bin/hc" "$HOME/.cargo/bin/herdr-core" "/opt/homebrew/bin/hc" "/opt/homebrew/bin/herdr-core" "/usr/local/bin/hc" "/usr/local/bin/herdr-core"; do
    if [ -n "$path" ] && [ -f "$path" ] && [ -x "$path" ]; then
        HC_SESSION={session}
        export HC_SESSION
        if "$path" server status --json 2>/dev/null | grep -qF {core_id}; then
            exec "$path" server stop
        fi
        exit {STOPPED_ELSEWHERE}
    fi
done
exit 127"#,
        core_id = word(core_id),
    );
    Ok(format!("/bin/sh -c {}", quote(&script)))
}

/// `text` as one shell word: bare when it is plain, else single-quoted.
fn word(text: &str) -> String {
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        text.to_owned()
    } else {
        quote(text)
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Spaces a poll found vacant, with the server each had and since when.
#[derive(Clone, Default)]
pub(super) struct Vacancies {
    seen: BTreeMap<String, (Instant, String)>,
    /// Spaces whose stop failed, by the server: not retried until it changes.
    failed: BTreeSet<(String, String)>,
}

impl Vacancies {
    /// Records one snapshot. `vacant` lists the `(space, core id)` pairs it
    /// found vacant; every other space is forgotten, and a space whose server
    /// changed starts again.
    pub(super) fn observe(&mut self, now: Instant, vacant: Vec<(String, String)>) {
        let names = vacant
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<BTreeSet<_>>();
        self.seen.retain(|name, _| names.contains(name.as_str()));
        self.failed
            .retain(|(name, id)| vacant.iter().any(|row| row == &(name.clone(), id.clone())));
        for (name, id) in &vacant {
            match self.seen.get(name) {
                Some((_, seen_id)) if seen_id == id => {}
                _ => {
                    self.seen.insert(name.clone(), (now, id.clone()));
                }
            }
        }
    }

    /// The spaces vacant in the last snapshot and in one at least `GRACE`
    /// older, whose stop did not fail: they are due to be stopped at `now`.
    pub(super) fn due(&self, now: Instant) -> Vec<(String, String)> {
        self.seen
            .iter()
            .filter(|(name, (since, id))| {
                now.saturating_duration_since(*since) >= GRACE
                    && !self.failed.contains(&((*name).clone(), id.clone()))
            })
            .map(|(name, (_, id))| (name.clone(), id.clone()))
            .collect()
    }

    /// The space's server stopped (or its row is gone): stop tracking it.
    pub(super) fn done(&mut self, name: &str) {
        self.seen.remove(name);
    }

    /// The stop failed: show the error once, retry when the server changes.
    pub(super) fn failed(&mut self, name: &str, core_id: &str) {
        self.failed.insert((name.to_owned(), core_id.to_owned()));
    }
}

/// When the connected space looks again at being vacant: at once when its last
/// tab goes, and every `GRACE` while another client's terminal holds it.
#[derive(Default)]
pub(super) struct Watch {
    armed: bool,
    retry: Option<Instant>,
}

impl Watch {
    /// True when the worker should take the fresh list and decide now.
    /// `gone` says the space's last tab is gone.
    pub(super) fn due(&mut self, gone: bool, now: Instant) -> bool {
        if !gone {
            *self = Self::default();
            return false;
        }
        if !self.armed {
            self.armed = true;
            return true;
        }
        match self.retry {
            Some(at) if now >= at => {
                self.retry = None;
                true
            }
            _ => false,
        }
    }

    /// A terminal still runs in the space: look again after `GRACE`.
    pub(super) fn busy(&mut self, now: Instant) {
        self.retry = Some(now + GRACE);
    }
}

/// Where selection goes when the selected space is removed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NextSpace {
    pub(crate) name: String,
    pub(crate) core_id: Option<String>,
}

/// A space the worker removed: its server is stopped and its row is gone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Removal {
    pub(crate) name: String,
    pub(crate) next: Option<NextSpace>,
}

/// The space that takes the removed one's place in the sidebar's order, else
/// the last one: the official app's rule when its selected space closes
/// (`herdr-app/src/model.rs` `remove_space`:
/// `self.spaces.get(pi).or(self.spaces.last())`). Only a confirmed space can
/// be connected to, so unconfirmed rows are passed over.
pub(super) fn next_space(order: &[SessionInfo], removed: &str) -> Option<NextSpace> {
    let at = order.iter().position(|row| row.name == removed)?;
    let rest = order
        .iter()
        .filter(|row| row.name != removed)
        .collect::<Vec<_>>();
    let row = rest
        .iter()
        .skip(at)
        .find(|row| row.confirmed)
        .or_else(|| rest.iter().rev().find(|row| row.confirmed))?;
    Some(NextSpace {
        name: row.name.clone(),
        core_id: row.core_id.clone(),
    })
}
