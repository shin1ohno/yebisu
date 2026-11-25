//! The discovery poll's snapshots of a host's Herdr 2 spaces: which rows the
//! sidebar lists, which empty spaces are due to be stopped (`vacate.rs`), and
//! which saved layouts belong to a server that is gone.
use super::{
    ExternalObservation, Observations, SessionList, store,
    vacate::{self, Expect, Stopped},
    worker,
};
use sheprd_core_client::wire::Terminal;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

/// What one discovery output says of each space of the host.
struct Snapshot {
    /// A server answered, or none is there (`None`: no socket, or nothing
    /// listens); both are rows of the sidebar.
    observed: Observations,
    /// Spaces whose server may be there but did not answer in time (or the
    /// row was unreadable): nothing is known of them.
    unknown: BTreeSet<String>,
    /// The names-only listing (no python3 on the host): it cannot say whether
    /// a listed space's server answers.
    names_only: bool,
}

pub(super) fn record_discovery(
    lists: &SessionList,
    root: Option<&Path>,
    host: &str,
    output: Option<&[u8]>,
) -> crate::Result<bool> {
    record_discovery_at(lists, root, host, output, Instant::now())
}

/// Takes one snapshot at `now`. `output` is `None` when the poll failed
/// (transport error or timeout): then nothing is deleted or forgotten. The
/// spaces found vacant are noted in `Discovery::vacancies`; `due_vacancies`
/// names those that stayed so long enough to be stopped.
pub(super) fn record_discovery_at(
    lists: &SessionList,
    root: Option<&Path>,
    host: &str,
    output: Option<&[u8]>,
    now: Instant,
) -> crate::Result<bool> {
    let Some(snapshot) = output.map(parse_discovery) else {
        return Ok(record_failure(lists));
    };
    let Snapshot {
        mut observed,
        unknown,
        names_only,
    } = snapshot;
    // Spaces holding a terminal whose state is not finished: never vacant,
    // whatever the saved tabs say (a state this client does not know counts).
    let busy = observed
        .iter()
        .filter(|(_, row)| {
            row.as_ref()
                .is_some_and(|row| !vacate::vacant(0, &row.terminals))
        })
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let pids = observed
        .iter()
        .filter_map(|(name, row)| Some((name.clone(), row.as_ref()?.pid)))
        .collect::<BTreeMap<_, _>>();
    let listed = observed
        .keys()
        .cloned()
        .chain(unknown.iter().cloned())
        .collect::<BTreeSet<_>>();
    // A space whose server did not answer is still a (unconfirmed) row.
    for name in &unknown {
        observed.entry(name.clone()).or_insert(None);
    }
    let rows = worker::catalog_sessions(root, host, observed)?;
    let mut guard = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current = guard.current.clone();
    let discovery = &mut guard.discovery;
    discovery.failed = false;
    let mut rows = rows;
    // A space stopped here stays off the rows while its server does not
    // answer, or answers as the one that was stopped (a poll that began before
    // the stop). It comes back when another client starts it again and it
    // runs a terminal: a different pid, or (pid not known) a live terminal.
    discovery.removed.retain(|name, stopped_pid| {
        let answering = pids.contains_key(name);
        let restarted = match stopped_pid {
            Some(pid) => pids
                .get(name)
                .is_some_and(|now| now.is_some_and(|n| n != *pid)),
            None => busy.contains(name),
        };
        listed.contains(name) && !(answering && restarted)
    });
    rows.retain(|row| !discovery.removed.contains_key(&row.name));
    // A saved layout whose server this successful poll does not find is a
    // space that is gone (stopped elsewhere, or by `hc server stop`): it
    // leaves the sidebar and its layout file is deleted. Only "absent" counts:
    // no socket, or nothing listening, in two successful polls in a row. A
    // server that did not answer in time (`unknown`) is neither: its row and
    // file stay.
    let mut gone = Vec::new();
    let mut absent = BTreeSet::new();
    for row in rows
        .iter()
        .filter(|row| !row.confirmed && row.name != current && !vacate::protected(&row.name))
    {
        if !listed.contains(&row.name) {
            gone.push(row.name.clone());
        } else if !names_only && !unknown.contains(&row.name) {
            if discovery.unanswered.contains(&row.name) {
                gone.push(row.name.clone());
            }
            absent.insert(row.name.clone());
        }
    }
    discovery.unanswered = absent;
    rows.retain(|row| !gone.contains(&row.name));
    let vacant = rows
        .iter()
        .filter(|row| {
            row.confirmed
                && !vacate::protected(&row.name)
                && !busy.contains(&row.name)
                && row.tabs.is_empty()
        })
        .filter_map(|row| {
            let id = row.core_id.clone()?;
            Some((
                row.name.clone(),
                Expect::seen(id, pids.get(&row.name).copied().flatten()),
            ))
        })
        .collect();
    discovery.vacancies.observe(now, vacant);
    let changed = discovery.rows.as_ref() != Some(&rows);
    if changed {
        discovery.rows = Some(rows);
    }
    drop(guard);
    if !gone.is_empty() {
        let _storage = store::LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for name in &gone {
            if let Err(error) = store::remove_workspace(root, host, name) {
                tracing::warn!(%error, %name, "Forget a Herdr 2 space that is gone");
            }
        }
    }
    Ok(changed || !gone.is_empty())
}

/// A failed poll (transport error or timeout) deletes nothing and forgets
/// nothing: rows and layouts stay. Two in a row mark every row unconfirmed.
fn record_failure(lists: &SessionList) -> bool {
    let mut lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let discovery = &mut lists.discovery;
    discovery.unanswered.clear();
    // A single transport failure leaves the last snapshot usable.
    if !std::mem::replace(&mut discovery.failed, true) {
        return false;
    }
    let Some(rows) = &mut discovery.rows else {
        return false;
    };
    let mut changed = false;
    for row in rows.iter_mut().filter(|row| row.confirmed) {
        row.mark_unconfirmed();
        changed = true;
    }
    changed
}

/// The spaces (name, server) the polls found vacant at two snapshots at least
/// `vacate::GRACE` apart, other than `current`, which its own worker removes.
pub(super) fn due_vacancies(
    lists: &SessionList,
    now: Instant,
    current: &str,
) -> Vec<(String, Expect)> {
    let lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    lists
        .discovery
        .vacancies
        .due(now)
        .into_iter()
        .filter(|(name, _)| name != current)
        .collect()
}

/// Stops one vacant space found by a poll and forgets it: its saved layout,
/// its row, its timer. The stop itself checks, on the host, that the server is
/// the one seen and that no terminal runs in it, so a terminal started since
/// the snapshot keeps the space (`Stopped::Busy`). On a failure the space stays
/// listed, the error is the result, and it is kept as the notice the screen
/// shows (`SessionLists::notice`). Blocking: a worker thread only.
pub(super) fn retire_vacant(
    lists: &SessionList,
    root: Option<&Path>,
    host: &str,
    (name, expect): &(String, Expect),
    stop: &vacate::Stopper,
) -> crate::Result<Stopped> {
    let stopped = match stop(name, expect) {
        Ok(stopped) => stopped,
        Err(error) => {
            let mut lists = lists
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lists.discovery.vacancies.failed(name, expect);
            lists.notice = Some(format!("Could not remove Herdr 2 space {name}: {error}"));
            return Err(error);
        }
    };
    let mut guard = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.discovery.vacancies.done(name);
    if stopped == Stopped::Busy {
        return Ok(stopped);
    }
    drop(guard);
    {
        let _storage = store::LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(error) = store::remove_workspace(root, host, name) {
            tracing::warn!(%error, %name, "Forget a removed Herdr 2 space");
        }
    }
    let mut lists = lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    lists.discovery.removed.insert(name.clone(), expect.pid);
    if let Some(rows) = &mut lists.discovery.rows {
        rows.retain(|row| &row.name != name);
    }
    // Its failure, if one was shown, is over with the space.
    if lists
        .notice
        .as_ref()
        .is_some_and(|notice| notice.contains(&format!("space {name}:")))
    {
        lists.notice = None;
    }
    Ok(stopped)
}

#[cfg(any(target_os = "ios", test))]
/// Every space a discovery output names, whatever its server's state: a
/// space is gone only when its host no longer lists it.
pub(super) fn listed_spaces(output: &[u8]) -> BTreeSet<String> {
    let Snapshot {
        observed, unknown, ..
    } = parse_discovery(output);
    observed.into_keys().chain(unknown).collect()
}

fn parse_discovery(output: &[u8]) -> Snapshot {
    let valid = |name: &str| {
        !name.is_empty()
            && name.trim() == name
            && !name.chars().any(char::is_control)
            && sheprd_client::core_connect_command(name).is_ok()
    };
    if let Some(names) = output.strip_prefix(b"\0") {
        return Snapshot {
            observed: names
                .split(|byte| *byte == 0)
                .filter_map(|field| std::str::from_utf8(field).ok())
                .filter(|name| valid(name))
                .map(|name| (name.to_owned(), None))
                .collect(),
            unknown: BTreeSet::new(),
            names_only: true,
        };
    }
    let mut observed = Observations::new();
    let mut unknown = BTreeSet::new();
    for line in output.split(|byte| *byte == b'\n') {
        let Ok(serde_json::Value::Array(row)) = serde_json::from_slice(line) else {
            continue;
        };
        let Some(name) = row.first().and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !valid(name) {
            continue;
        }
        match row.get(1) {
            // No socket, or nothing listens: the server is absent.
            Some(serde_json::Value::Null) => {
                observed.insert(name.to_owned(), None);
            }
            Some(serde_json::Value::Array(server)) => match observation(server) {
                Some(server) => {
                    observed.insert(name.to_owned(), Some(server));
                }
                None => {
                    unknown.insert(name.to_owned());
                }
            },
            // "unknown", or a row this client cannot read.
            _ => {
                unknown.insert(name.to_owned());
            }
        }
    }
    Snapshot {
        observed,
        unknown,
        names_only: false,
    }
}

/// `[core id, terminals, pid?]`.
fn observation(server: &[serde_json::Value]) -> Option<ExternalObservation> {
    let core_id = server.first()?.as_str().filter(|id| !id.is_empty())?;
    let terminals = serde_json::from_value::<Vec<Terminal>>(server.get(1)?.clone()).ok()?;
    Some(ExternalObservation {
        core_id: core_id.to_owned(),
        terminals,
        pid: server.get(2).and_then(serde_json::Value::as_u64),
    })
}

/// A row of the host's spaces, for tests that read the discovery rows.
#[cfg(test)]
pub(super) fn listed_rows(lists: &SessionList) -> Vec<super::SessionInfo> {
    lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .discovery
        .rows
        .clone()
        .unwrap_or_default()
}
