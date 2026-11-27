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
//! - the stop itself, on the host, asks the server (its socket, never `hc
//!   server status`, which starts a server) for its core id, its pid, its
//!   uptime and its live terminals, in the one connection it stops through,
//!   and refuses unless it is the server that was seen and runs nothing. A
//!   core id never changes (herdr-core keeps it for good), so the pid, or the
//!   age of the connection that saw the server, tells a restart.
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

/// The server a stop is for: what was seen of it, which the stop checks.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Expect {
    pub(crate) core_id: String,
    /// The pid of the server that was seen (a poll saw it), if known.
    pub(crate) pid: Option<u64>,
    /// The server must have run at least this long (the connection that saw
    /// it is that old), if known.
    pub(crate) min_uptime_ms: Option<u64>,
    /// Close Space of a space this client is not connected to (#249): the
    /// live terminals that were seen, which the stop ends first in its one
    /// checked connection. Another live terminal refuses the stop (`Busy`).
    /// Empty for an empty space: nothing is ended.
    pub(crate) end: Vec<String>,
}

impl Expect {
    pub(super) fn seen(core_id: String, pid: Option<u64>) -> Self {
        Self {
            core_id,
            pid,
            min_uptime_ms: None,
            end: Vec::new(),
        }
    }
}

/// What a stop did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Stopped {
    /// The server is stopped, or there was none (nothing was started).
    Done,
    /// A terminal that is not finished runs in it: it was left running.
    Busy,
}

/// Stops the server of `session` if it is the one `Expect` describes and no
/// terminal runs in it.
pub(crate) type Stopper = Arc<dyn Fn(&str, &Expect) -> crate::Result<Stopped> + Send + Sync>;

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
    Arc::new(move |session, expect| {
        let command = stop_command(session, expect)?;
        let (exit, output) =
            sheprd_client::run_ssh_command(&host, &command, Duration::from_secs(10), 4096, || {
                false
            })
            .map_err(|error| crate::Error::Ssh(error.to_string()))?;
        stop_result(exit.code(), &output)
    })
}

#[cfg(not(target_os = "ios"))]
fn ssh_stopper(_host: &str) -> Stopper {
    Arc::new(|_, _| {
        Err(crate::Error::Ssh(
            "SSH unavailable on this host: cannot stop a Herdr 2 space".into(),
        ))
    })
}

/// What the exit status and output of `stop_command` mean.
#[cfg_attr(not(any(target_os = "ios", test)), allow(dead_code))]
pub(crate) fn stop_result(code: Option<i32>, output: &[u8]) -> crate::Result<Stopped> {
    let said = crate::shell::herdr2_frame::session::one_line(&String::from_utf8_lossy(output));
    match code {
        Some(0) => Ok(Stopped::Done),
        Some(STILL_RUNNING) => Ok(Stopped::Busy),
        Some(OTHER_SERVER) => Err(crate::Error::Ssh(format!(
            "Herdr 2 space is not the server that was seen ({said}); left running"
        ))),
        Some(UNKNOWN) => Err(crate::Error::Ssh(
            "Herdr 2 server did not answer; left running".into(),
        )),
        Some(NO_PYTHON) => Err(crate::Error::Ssh("python3 is needed on the host".into())),
        _ => Err(crate::Error::Ssh(format!(
            "Herdr 2 server did not stop: {said}"
        ))),
    }
}

/// Exit statuses of `stop.py` (see its header).
const OTHER_SERVER: i32 = 3;
const STILL_RUNNING: i32 = 4;
const UNKNOWN: i32 = 5;
const NO_PYTHON: i32 = 6;

/// The remote POSIX command that stops one space's server: `stop.py` on the
/// host's own Python, which asks the session's socket (never starting a server)
/// for the core id, pid, uptime and live count and sends `server.stop` (the
/// request `hc server stop` sends, herdr-core `api.rs` `server_stop`) in the
/// same connection, only if it is the server in `expect` and nothing runs.
#[cfg_attr(not(any(target_os = "ios", test)), allow(dead_code))]
pub(crate) fn stop_command(session: &str, expect: &Expect) -> crate::Result<String> {
    // Validates the session name the way the connect command does.
    sheprd_client::core_connect_command(session).map_err(|e| crate::Error::Ssh(e.to_string()))?;
    if expect.core_id.is_empty()
        || !expect
            .core_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(crate::Error::Ssh("Herdr 2 core id is unusable".into()));
    }
    let number = |value: Option<u64>| value.map_or_else(|| "-".to_owned(), |n| n.to_string());
    if expect.end.iter().any(|address| {
        address.is_empty()
            || !address
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    }) {
        return Err(crate::Error::Ssh(
            "Herdr 2 terminal address is unusable".into(),
        ));
    }
    let end = if expect.end.is_empty() {
        "-".to_owned()
    } else {
        expect.end.join(",")
    };
    Ok(format!(
        r#"if ! command -v python3 >/dev/null 2>&1; then
    echo 'python3 is needed'
    exit {NO_PYTHON}
fi
exec python3 -I - {session} {id} {pid} {uptime} {end} <<'YEBISU_CORE_STOP'
{script}
YEBISU_CORE_STOP
"#,
        session = word(session),
        id = word(&expect.core_id),
        pid = number(expect.pid),
        uptime = number(expect.min_uptime_ms),
        end = word(&end),
        script = include_str!("../shell/herdr2_frame/stop.py"),
    ))
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
    seen: BTreeMap<String, (Instant, Expect)>,
    /// Spaces whose stop failed, by the server: not retried until it changes.
    failed: BTreeSet<(String, Expect)>,
}

impl Vacancies {
    /// Records one snapshot. `vacant` lists the `(space, server)` pairs it
    /// found vacant; every other space is forgotten, and a space whose server
    /// changed (another id or pid) starts again.
    pub(super) fn observe(&mut self, now: Instant, vacant: Vec<(String, Expect)>) {
        let names = vacant
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<BTreeSet<_>>();
        self.seen.retain(|name, _| names.contains(name.as_str()));
        self.failed.retain(|row| vacant.contains(row));
        for (name, expect) in &vacant {
            match self.seen.get(name) {
                Some((_, seen)) if seen == expect => {}
                _ => {
                    self.seen.insert(name.clone(), (now, expect.clone()));
                }
            }
        }
    }

    /// The spaces vacant in the last snapshot and in one at least `GRACE`
    /// older, whose stop did not fail: they are due to be stopped at `now`.
    pub(super) fn due(&self, now: Instant) -> Vec<(String, Expect)> {
        self.seen
            .iter()
            .filter(|(name, (since, expect))| {
                now.saturating_duration_since(*since) >= GRACE
                    && !self.failed.contains(&((*name).clone(), expect.clone()))
            })
            .map(|(name, (_, expect))| (name.clone(), expect.clone()))
            .collect()
    }

    /// The space's server stopped (or its row is gone): stop tracking it.
    pub(super) fn done(&mut self, name: &str) {
        self.seen.remove(name);
    }

    /// The stop failed: say so once, retry when the server changes.
    pub(super) fn failed(&mut self, name: &str, expect: &Expect) {
        self.failed.insert((name.to_owned(), expect.clone()));
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
