//! Lifecycle policy contains no I/O. Each selected host owns a session counter;
//! Link's separate monotonic token also fences callbacks across host switches.
use crate::{app::Smoke, live::Live};
use gpui::{AppLifecyclePhase, Context};
use std::collections::BTreeMap;

pub(crate) struct Reconnect {
    pub automatic: bool,
    pub suspended: bool,
    pub target: String,
    generations: BTreeMap<String, u64>,
}
impl Default for Reconnect {
    fn default() -> Self {
        Self {
            automatic: true,
            suspended: false,
            target: String::new(),
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
        crate::diagnostics::record(match phase {
            AppLifecyclePhase::Active => "active",
            AppLifecyclePhase::Foreground => "foreground",
            AppLifecyclePhase::Inactive => "inactive",
            AppLifecyclePhase::Background => "background",
            AppLifecyclePhase::Disconnected => "scene_disconnected",
        });
        match phase {
            AppLifecyclePhase::Inactive
            | AppLifecyclePhase::Background
            | AppLifecyclePhase::Disconnected => {
                if self.terminal.reconnect.suspended {
                    return;
                }
                self.handoff.cancel();
                self.handoff.selection = None;
                self.terminal.reconnect.suspended = true;
                for terminal in self.board.hidden.values_mut() {
                    terminal.reconnect_pending = false;
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
                for terminal in self.board.hidden.values_mut() {
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
                if returned
                    && self.terminal.open
                    && self.terminal.reconnect.automatic
                    && self.terminal.live.version_mismatch.get().is_none()
                {
                    self.connect_terminal(cx);
                }
                for host in hosts {
                    self.start_host_connection(host, cx);
                }
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                if returned && let Some(release) = self.terminal.late_reader_release.take() {
                    // The R5 peer's old reader emits only AFTER Automatic tried
                    // to reconnect. This cannot be compiled into a device build.
                    self.observation.record(serde_json::json!({
                        "kind":"r5-reader-release", "pending":self.terminal.reconnect_pending,
                        "automatic":self.terminal.reconnect.automatic,
                        "generation":self.terminal.link.generation
                    }));
                    let _ = release.send(());
                }
            }
            AppLifecyclePhase::Foreground => {}
        }
        self.observation.record(serde_json::json!({"kind":"lifecycle","phase":format!("{phase:?}"),"generation":self.terminal.link.generation,"suspended":self.terminal.reconnect.suspended}));
        cx.notify();
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
