//! Lifecycle policy contains no I/O. Each selected host owns a session counter;
//! Link's separate monotonic token also fences callbacks across host switches.
use crate::{app::Smoke, live::Live};
use gpui::{AppLifecyclePhase, Context};
use std::collections::BTreeMap;

pub(crate) struct Reconnect {
    pub automatic: bool,
    pub suspended: bool,
    pub target: String,
    pub core_session_on_return: Option<String>,
    generations: BTreeMap<String, u64>,
}
impl Default for Reconnect {
    fn default() -> Self {
        Self {
            automatic: true,
            suspended: false,
            target: String::new(),
            core_session_on_return: None,
            generations: BTreeMap::new(),
        }
    }
}
impl Reconnect {
    pub fn current(&self, host: &str, generation: u64) -> bool {
        self.target == host && self.generations.get(host) == Some(&generation)
    }
    pub fn begin(&mut self, target: String) -> u64 {
        self.target = target;
        let generation = self.generations.entry(self.target.clone()).or_default();
        *generation = generation.wrapping_add(1);
        *generation
    }
}
impl Smoke {
    pub(crate) fn lifecycle(&mut self, phase: AppLifecyclePhase, cx: &mut Context<Self>) {
        self.input_transition(|this| this.lifecycle_inner(phase, cx));
        // Outside `lifecycle_inner`: its early return for a second Background
        // or a Disconnected while already suspended must not skip the flush.
        self.flush_before_suspension(phase);
        // Outside it as well, so that early return cannot keep the display lit.
        self.keep_awake.lifecycle(phase);
        self.sync_keep_awake(cx);
    }

    /// UIKit may suspend the process right after Inactive or Background, and
    /// a suspended app can be killed or terminated: records still queued die
    /// with it and the next process cannot tell. Put them in the file first;
    /// a writer that is not done in time is said, in stderr and in a record
    /// sent without waiting for room. QA writers only (`Observation::flush`).
    fn flush_before_suspension(&self, phase: AppLifecyclePhase) {
        if !matches!(
            phase,
            AppLifecyclePhase::Inactive
                | AppLifecyclePhase::Background
                | AppLifecyclePhase::Disconnected
        ) {
            return;
        }
        if let Err(pending) = self.observation.flush(crate::observation::LIFECYCLE_FLUSH) {
            eprintln!(
                "smoke observation flush timed out: {pending} records unwritten at {phase:?}"
            );
            self.observation.record_without_waiting(serde_json::json!({
                "kind": "observation-flush-timeout",
                "phase": format!("{phase:?}"),
                "pending": pending,
            }));
        }
    }
    fn lifecycle_inner(&mut self, phase: AppLifecyclePhase, cx: &mut Context<Self>) {
        self.settings_lifecycle(phase);
        self.notification_lifecycle(phase);
        crate::diagnostics::record(match phase {
            AppLifecyclePhase::Active => "active",
            AppLifecyclePhase::Foreground => "foreground",
            AppLifecyclePhase::Inactive => "inactive",
            AppLifecyclePhase::Background => "background",
            AppLifecyclePhase::Disconnected => "scene_disconnected",
        });
        match phase {
            AppLifecyclePhase::Background | AppLifecyclePhase::Disconnected => {
                if self.terminal.reconnect.suspended {
                    return;
                }
                self.handoff.cancel();
                self.handoff.selection = None;
                self.terminal.reconnect.suspended = true;
                for terminal in self
                    .board
                    .hidden
                    .values_mut()
                    .chain(self.board.hidden_core.values_mut())
                {
                    terminal.reconnect_pending = false;
                    terminal.reconnect.core_session_on_return =
                        terminal.core.take().map(|core| core.name().to_owned());
                    terminal.link.pause();
                    terminal.input.detach();
                    terminal.live.connected = false;
                    terminal.restore_stop_reason();
                    if terminal.live.version_mismatch.get().is_none() {
                        terminal.live.status = "Paused".into();
                    }
                    terminal.reconnect.suspended = true;
                    terminal.navigating = None;
                    terminal.link.inbox =
                        std::sync::Arc::new(std::sync::Mutex::new(terminal.live.clone()));
                }
                if self.terminal.open || !self.terminal.reconnect.target.is_empty() {
                    let mismatch = self.terminal.link.stop_reason.clone();
                    self.terminal.reconnect_pending = false;
                    self.terminal.core.take();
                    self.terminal.link.pause();
                    self.terminal
                        .input
                        .sync(&Live::default(), self.terminal.link.generation, None);
                    self.terminal.live = Live::default();
                    self.terminal.live.version_mismatch = mismatch;
                    self.terminal.live.status =
                        "Paused · input discarded · reconnect on return".into();
                    self.terminal.restore_stop_reason();
                    self.terminal.link.inbox =
                        std::sync::Arc::new(std::sync::Mutex::new(self.terminal.live.clone()));
                    self.terminal.size = None;
                }
            }
            AppLifecyclePhase::Active => {
                self.terminal.restore_stop_reason();
                for terminal in self
                    .board
                    .hidden
                    .values_mut()
                    .chain(self.board.hidden_core.values_mut())
                {
                    terminal.restore_stop_reason();
                }
                let returned = std::mem::replace(&mut self.terminal.reconnect.suspended, false);
                // Capture only hosts suspended before connect_terminal creates new entries.
                let hosts = self
                    .board
                    .hidden
                    .iter_mut()
                    .filter_map(|(host, t)| {
                        let suspended = std::mem::replace(&mut t.reconnect.suspended, false);
                        (returned
                            && suspended
                            && t.reconnect.automatic
                            && t.live.version_mismatch.get().is_none())
                        .then(|| host.clone())
                    })
                    .collect::<Vec<_>>();
                let core_sessions = self
                    .board
                    .hidden_core
                    .iter_mut()
                    .filter_map(|(host, terminal)| {
                        let reconnect = &mut terminal.reconnect;
                        let suspended = std::mem::replace(&mut reconnect.suspended, false);
                        let name = reconnect.core_session_on_return.take()?;
                        (returned
                            && suspended
                            && reconnect.automatic
                            && terminal.live.version_mismatch.get().is_none())
                        .then(|| (host.clone(), name))
                    })
                    .collect::<Vec<_>>();
                if returned
                    && self.terminal.open
                    && self.terminal.reconnect.automatic
                    && self.terminal.live.version_mismatch.get().is_none()
                {
                    self.connect_terminal(cx);
                }
                for host in hosts {
                    self.start_herdr1_connection(host, cx);
                }
                for (host, name) in core_sessions {
                    self.resume_hidden_core(host, name, cx);
                }
            }
            // Losing scene focus is not backgrounding: retain both transports,
            // their input destinations and any staged clipboard ownership.
            AppLifecyclePhase::Inactive | AppLifecyclePhase::Foreground => {}
        }
        self.observation.record(serde_json::json!({"kind":"lifecycle","phase":format!("{phase:?}"),"generation":self.terminal.link.generation,"suspended":self.terminal.reconnect.suspended,"connected":self.terminal.live.connected,"herdr1":self.terminal.link.handle.is_some(),"core":self.terminal.core.is_some()}));
        cx.notify();
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "inactive_tests.rs"]
mod inactive_tests;
