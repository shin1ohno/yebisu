//! Cold-launch restore (owner decision 2026-11-25): a new process reopens the
//! connections the last one had open, the shown one and the hidden ones, the
//! same scope as a background return (`reconnect.rs`). `Return: Automatic`
//! dials each once; `Return: Tap reconnect` only shows the saved terminal,
//! which waits for Reconnect. A failure shows its reason and waits for
//! Reconnect; a saved Herdr 2 space that is gone opens `default` with a
//! notice. Saved hosts are the only destinations: an entry for a host that is
//! no longer saved is dropped.
//!
//! The record is written off the UI thread whenever what is open changes
//! while the app is active (`store::Writer`), so a process that is suspended
//! and killed has already left it.
mod store;

use crate::{
    actions::KeyStatus,
    app::Smoke,
    shell::{HerdrKind, herdr2_frame::HERDR2_SESSION},
};
use gpui::Context;
use std::collections::BTreeSet;

/// The status of a restored terminal that waits for Reconnect.
pub(crate) const TAP_RECONNECT: &str = "Not connected · tap Reconnect";

/// What was open: the shown terminal, then the hidden ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) selected: Option<Entry>,
    pub(crate) others: Vec<Entry>,
}

/// One connection: a saved host, its Herdr and, for Herdr 2, the space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) space: Option<String>,
}

#[derive(Default)]
enum Phase {
    /// Not started: host tests and QA fixtures neither restore nor write.
    #[default]
    Idle,
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    Loading,
    Loaded(Option<Record>),
    Done,
}

/// Where a restored Herdr 2 space goes once its host has listed its spaces.
#[derive(Clone, Copy)]
enum Place {
    /// The shown terminal, while the user has not moved on (`board.selection`).
    Selected(u64),
    Hidden,
}

#[cfg(test)]
type SpaceProbe = dyn Fn(&str) -> crate::Result<BTreeSet<String>> + Send + Sync;

#[derive(Default)]
pub(crate) struct LaunchRestore {
    phase: Phase,
    /// The record last handed to the writer.
    written: Option<Record>,
    writer: Option<store::Writer>,
    /// Space checks still running: the record is not rewritten meanwhile.
    pending: usize,
    #[cfg(test)]
    pub(crate) probe: Option<std::sync::Arc<SpaceProbe>>,
}

impl LaunchRestore {
    #[cfg(test)]
    pub(crate) fn loaded(record: Option<Record>) -> Self {
        Self {
            phase: Phase::Loaded(record),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn written(&self) -> Option<&Record> {
        self.written.as_ref()
    }
}

/// A Simulator QA launch drives its own fixture; a record left by an earlier
/// QA run in the same container must not reopen anything under it.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn qa_launch() -> bool {
    cfg!(all(target_os = "ios", target_abi = "sim"))
        && qa_environment(std::env::vars_os().map(|(key, _)| key))
}

/// Every XCTest launch sets a `YEBISU_` key, and not every set has a
/// `_QA` one (EditMenuQA: `YEBISU_QA_AX`, `YEBISU_SMOKE_OBSERVE`,
/// `YEBISU_SH1_152_CASE`). `scripts/ios/sim.sh` sets none, so a manual
/// Simulator launch still restores.
fn qa_environment(keys: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    keys.into_iter()
        .any(|key| key.to_str().is_some_and(|key| key.starts_with("YEBISU_")))
}

fn probe_spaces(host: &str) -> crate::Result<BTreeSet<String>> {
    #[cfg(target_os = "ios")]
    {
        crate::core::probe_spaces(host)
    }
    #[cfg(not(target_os = "ios"))]
    {
        Err(crate::Error::Ssh(format!(
            "SSH unavailable in host test: {host}"
        )))
    }
}

impl Smoke {
    /// Launch: read the record off the UI thread (`app.rs`, with `Job::Load`).
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(crate) fn start_launch_restore(&mut self, cx: &mut Context<Self>) {
        if qa_launch() {
            return;
        }
        let Some(root) = sheprd_client::local_path::state_dir() else {
            return;
        };
        self.launch.phase = Phase::Loading;
        match store::Writer::start(root.clone()) {
            Ok(writer) => self.launch.writer = Some(writer),
            Err(error) => tracing::warn!(%error, "launch record writer"),
        }
        let read = cx
            .background_executor()
            .spawn(async move { store::load(&root) });
        cx.spawn(async move |weak, cx| {
            let result = read.await;
            let _ = weak.update(cx, |this, _| {
                let record = result
                    .inspect_err(|error| tracing::warn!(%error, "launch record unreadable"))
                    .ok()
                    .flatten();
                this.launch.phase = Phase::Loaded(record);
            });
        })
        .detach();
    }

    /// Every poll (`poll_terminal`): restore once both the saved hosts and the
    /// Return setting are known, then keep the record current.
    pub(crate) fn poll_launch_restore(&mut self, cx: &mut Context<Self>) {
        if self.terminal.reconnect.suspended {
            return;
        }
        match &self.launch.phase {
            Phase::Loaded(_) if self.launch_inputs_ready() => {
                let Phase::Loaded(record) = std::mem::replace(&mut self.launch.phase, Phase::Done)
                else {
                    return;
                };
                if let Some(record) = record {
                    self.restore_launch(record, cx);
                }
            }
            Phase::Done if self.launch.pending == 0 => self.save_launch_snapshot(),
            _ => {}
        }
    }

    fn launch_inputs_ready(&self) -> bool {
        self.setup.loaded
            && !self.setup.busy
            && self
                .settings
                .as_ref()
                .is_none_or(crate::settings::SettingsHost::loaded)
    }

    fn save_launch_snapshot(&mut self) {
        let record = self.launch_snapshot();
        if self.launch.written.as_ref() == Some(&record) {
            return;
        }
        if let Some(writer) = &self.launch.writer {
            writer.send(record.clone());
        }
        self.launch.written = Some(record);
    }

    /// What is open now, limited to saved hosts.
    pub(crate) fn launch_snapshot(&self) -> Record {
        let saved = |host: &str| self.setup.hosts.iter().any(|saved| saved == host);
        let target = self.terminal.reconnect.target.as_str();
        let selected = (self.terminal.open && saved(target)).then(|| {
            let kind = self.terminal.herdr_kind();
            Entry {
                host: target.to_owned(),
                kind,
                space: (kind == HerdrKind::Herdr2).then(|| {
                    self.terminal.core.as_ref().map_or_else(
                        || self.shell.herdr2.selected_session(target).to_owned(),
                        |core| core.name().to_owned(),
                    )
                }),
            }
        });
        let herdr1 = self.board.hidden.keys().map(|host| Entry {
            host: host.clone(),
            kind: HerdrKind::Herdr1,
            space: None,
        });
        let herdr2 = self.board.hidden_core.iter().filter_map(|(host, view)| {
            let space = view.core.as_ref().map_or_else(
                || view.reconnect.core_session_on_return.clone(),
                |core| Some(core.name().to_owned()),
            )?;
            Some(Entry {
                host: host.clone(),
                kind: HerdrKind::Herdr2,
                space: Some(space),
            })
        });
        let others = herdr1
            .chain(herdr2)
            .filter(|entry| saved(&entry.host))
            .filter(|entry| {
                selected.as_ref().is_none_or(|shown| {
                    (shown.host.as_str(), shown.kind) != (&entry.host, entry.kind)
                })
            })
            .collect();
        Record { selected, others }
    }

    fn restore_launch(&mut self, record: Record, cx: &mut Context<Self>) {
        // The user opened or is adding a host before the record was usable.
        if self.terminal.open || self.shell.hosts_sheet.add.is_some() {
            return;
        }
        let saved = |entry: &Entry| self.setup.hosts.contains(&entry.host);
        let selected = record.selected.filter(saved);
        let others = record.others.into_iter().filter(saved).collect::<Vec<_>>();
        let automatic = self.terminal.reconnect.automatic;
        self.observation.record(serde_json::json!({
            "kind": "launch-restore", "automatic": automatic,
            "selected": selected.as_ref().map(|entry| entry.host.clone()),
            "others": others.len(),
        }));
        if let Some(entry) = selected {
            self.shell.hosts_sheet.close();
            self.restore_selected(entry, automatic, cx);
        }
        if !automatic {
            return;
        }
        for entry in others {
            match entry.kind {
                HerdrKind::Herdr1 => self.start_herdr1_connection(entry.host, cx),
                HerdrKind::Herdr2 => {
                    let space = entry.space.unwrap_or_else(|| HERDR2_SESSION.to_owned());
                    self.restore_space(entry.host, space, Place::Hidden, cx);
                }
            }
        }
        cx.notify();
    }

    fn restore_selected(&mut self, entry: Entry, automatic: bool, cx: &mut Context<Self>) {
        let Entry { host, kind, space } = entry;
        if kind == HerdrKind::Herdr2 {
            let space = space.unwrap_or_else(|| HERDR2_SESSION.to_owned());
            // Reconnect (`connect_core`) reopens this space, not `default`.
            self.shell.herdr2.confirm_session(&host, &space);
            self.select_host_kind(&host, kind);
            if automatic {
                self.terminal.live.status = format!("Opening space {space}…");
                self.restore_space(host, space, Place::Selected(self.board.selection), cx);
            } else {
                self.terminal.live.status = TAP_RECONNECT.into();
            }
            return;
        }
        self.select_host_kind(&host, kind);
        if automatic {
            self.start_herdr1_connection(host, cx);
        } else {
            self.terminal.live.status = TAP_RECONNECT.into();
        }
    }

    /// `default` always opens; another space first has its host list its
    /// spaces, because connecting to a name that is gone would make a new,
    /// empty space under it.
    fn restore_space(&mut self, host: String, space: String, place: Place, cx: &mut Context<Self>) {
        if space == HERDR2_SESSION {
            self.open_restored_space(&host, space, None, place, cx);
            return;
        }
        #[cfg(test)]
        let probe = self.launch.probe.clone();
        let listed = {
            let host = host.clone();
            cx.background_executor().spawn(async move {
                #[cfg(test)]
                if let Some(probe) = probe {
                    return probe(&host);
                }
                probe_spaces(&host)
            })
        };
        self.launch.pending += 1;
        cx.spawn(async move |weak, cx| {
            let listed = listed.await;
            let _ = weak.update(cx, |this, cx| {
                this.launch.pending = this.launch.pending.saturating_sub(1);
                match listed {
                    Ok(spaces) if spaces.contains(&space) => {
                        this.open_restored_space(&host, space, None, place, cx);
                    }
                    Ok(_) => {
                        this.shell.herdr2.forget_session(&host, &space);
                        this.shell.herdr2.confirm_session(&host, HERDR2_SESSION);
                        let notice = format!("Space {space} is gone · opened {HERDR2_SESSION}");
                        this.open_restored_space(
                            &host,
                            HERDR2_SESSION.to_owned(),
                            Some(notice),
                            place,
                            cx,
                        );
                    }
                    Err(error) => this.fail_restored_space(&host, &space, &error, place),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn still_waiting(&self, host: &str, place: Place) -> bool {
        match place {
            Place::Selected(selection) => {
                self.board.selection == selection
                    && self.terminal.reconnect.target == host
                    && self.terminal.core_mode
                    && self.terminal.core.is_none()
            }
            Place::Hidden => {
                !(self.terminal.reconnect.target == host && self.terminal.core_mode)
                    && self
                        .board
                        .hidden_core
                        .get(host)
                        .is_none_or(|view| view.core.is_none())
            }
        }
    }

    fn open_restored_space(
        &mut self,
        host: &str,
        space: String,
        notice: Option<String>,
        place: Place,
        cx: &mut Context<Self>,
    ) {
        if !self.still_waiting(host, place) {
            return;
        }
        match place {
            Place::Selected(_) => {
                if self
                    .connect_named_core(host.to_owned(), space, None, cx)
                    .is_ok()
                    && let Some(text) = notice
                {
                    self.actions.key_status =
                        Some(KeyStatus::new(text, &self.terminal, self.board.selection));
                }
            }
            Place::Hidden => {
                self.board.hidden_core.entry(host.to_owned()).or_default();
                self.resume_hidden_core(host.to_owned(), space, cx);
                if let (Some(text), Some(view)) = (notice, self.board.hidden_core.get_mut(host)) {
                    view.live.status = text;
                }
            }
        }
    }

    fn fail_restored_space(&mut self, host: &str, space: &str, error: &crate::Error, place: Place) {
        if !self.still_waiting(host, place) {
            return;
        }
        let reason = format!("Space {space} not restored: {error}");
        match place {
            Place::Selected(_) => self.terminal.live.status = reason,
            Place::Hidden => {
                let view = self.board.hidden_core.entry(host.to_owned()).or_default();
                view.core_mode = true;
                view.reconnect.target = host.to_owned();
                // Kept in the record while the app stays active.
                view.reconnect.core_session_on_return = Some(space.to_owned());
                view.live.status = reason;
            }
        }
    }
}

#[cfg(test)]
#[path = "launch_restore_tests.rs"]
mod tests;
