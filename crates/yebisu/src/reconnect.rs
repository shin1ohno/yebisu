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
        match phase {
            AppLifecyclePhase::Inactive
            | AppLifecyclePhase::Background
            | AppLifecyclePhase::Disconnected => {
                if self.terminal.reconnect.suspended {
                    return;
                }
                self.terminal.reconnect.suspended = true;
                if self.terminal.open {
                    let mismatch = self.terminal.live.version_mismatch.clone();
                    self.terminal.link.reset();
                    self.terminal
                        .input
                        .sync(&Live::default(), self.terminal.link.generation, None);
                    self.terminal.live = Live::default();
                    self.terminal.live.version_mismatch = mismatch;
                    self.terminal.live.status =
                        "Paused · input discarded · reconnect on return".into();
                    self.terminal.link.inbox =
                        std::sync::Arc::new(std::sync::Mutex::new(self.terminal.live.clone()));
                    self.terminal.size = None;
                }
            }
            AppLifecyclePhase::Active => {
                let returned = std::mem::replace(&mut self.terminal.reconnect.suspended, false);
                if returned
                    && self.terminal.open
                    && self.terminal.reconnect.automatic
                    && self.terminal.live.version_mismatch.is_none()
                {
                    self.connect_terminal(cx);
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
