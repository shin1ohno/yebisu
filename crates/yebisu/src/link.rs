//! One reader per connection; reconnect replaces all mailboxes before starting.
use crate::live::Live;
use sheprd_client::{Client, ClientEvent, ClientHandle};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

pub(crate) struct Link {
    pub generation: u64,
    pub stop_reason: Arc<OnceLock<sheprd_client::VersionMismatch>>,
    pub reader: Option<JoinHandle<()>>,
    retired_readers: Vec<JoinHandle<()>>,
    #[cfg(test)]
    pub before_dial: Option<Box<dyn FnOnce()>>,
    pub inbox: Arc<Mutex<Live>>,
    pub drained: Arc<AtomicBool>,
    pub handle: Option<ClientHandle>,
}
impl Default for Link {
    fn default() -> Self {
        let live = Live::default();
        Self {
            generation: 0,
            stop_reason: live.version_mismatch.clone(),
            reader: None,
            retired_readers: Vec::new(),
            #[cfg(test)]
            before_dial: None,
            inbox: Arc::new(Mutex::new(live)),
            drained: Arc::default(),
            handle: None,
        }
    }
}
impl Link {
    pub fn reset(&mut self) -> u64 {
        self.pause()
    }
    /// A core connection belongs to a different protocol stop-state domain.
    /// Retired Herdr 1 readers keep their own Arc; never clear or reuse it here.
    pub fn reset_for_core(&mut self) -> u64 {
        self.stop_reason = Arc::new(OnceLock::new());
        self.pause()
    }
    /// Invalidate callbacks and payloads without losing the reader's stop reason.
    pub fn pause(&mut self) -> u64 {
        if let Some(handle) = self.handle.take() {
            handle.disconnect();
        }
        if let Some(reader) = self.reader.take() {
            self.retired_readers.push(reader);
        } // Never join on the UI thread.
        self.generation = self.generation.wrapping_add(1);
        let mut live = Live::default();
        live.version_mismatch = self.stop_reason.clone();
        live.status = "Connecting…".into();
        live.dirty = true;
        self.inbox = Arc::new(Mutex::new(live));
        self.drained = Arc::new(AtomicBool::new(false));
        self.generation
    }
    /// Dial permission and stop retention are one contract. Once a reader has
    /// finished, no further writes to its shared stop reason are possible.
    pub fn prepare_connection(&mut self) -> Option<u64> {
        if self.handle.is_some() || self.reader.is_some() {
            self.pause();
        }
        #[cfg(test)]
        if let Some(inject) = self.before_dial.take() {
            inject();
        }
        self.retired_readers.retain(|reader| !reader.is_finished());
        if !self.retired_readers.is_empty() || self.stop_reason.get().is_some() {
            return None;
        }
        crate::diagnostics::record("connecting");
        Some(self.reset())
    }
    pub fn install(&mut self, generation: u64, result: crate::Result<Client>) {
        if generation != self.generation {
            if let Ok(client) = result {
                client.handle.disconnect();
            }
            return;
        }
        match result {
            Err(e) => self.startup_failed(e.to_string()),
            Ok(client) => {
                self.handle = Some(client.handle);
                let inbox = self.inbox.clone();
                let drained = self.drained.clone();
                match std::thread::Builder::new()
                    .name("yebisu-events".into())
                    .spawn(move || {
                        while let Ok(event) = client.events.recv() {
                            if let Ok(mut live) = inbox.lock() {
                                live.apply(event);
                            }
                        }
                        drained.store(true, Ordering::Release);
                    }) {
                    Ok(reader) => self.reader = Some(reader),
                    Err(e) => {
                        if let Some(handle) = self.handle.take() {
                            handle.disconnect();
                        }
                        self.startup_failed(e.to_string());
                    }
                }
            }
        }
    }
    fn startup_failed(&self, reason: String) {
        crate::diagnostics::record("connection_error");
        if let Ok(mut live) = self.inbox.lock() {
            live.apply(ClientEvent::Disconnected { reason, ssh: None });
        }
        self.drained.store(true, Ordering::Release);
    }
    pub fn take_update(&self) -> Option<Live> {
        let mut live = self.inbox.try_lock().ok()?;
        if let Some(reason) = self.stop_reason.get() {
            let status = format!("Version mismatch: {reason:?}");
            if live.status != status {
                live.status = status;
                live.dirty = true;
            }
        }
        if !live.dirty {
            return None;
        }
        live.dirty = false;
        let clipboard = std::mem::take(&mut live.clipboard);
        let mut result = live.clone();
        result.clipboard = clipboard;
        Some(result)
    }
    pub fn current(&self, generation: u64) -> bool {
        self.generation == generation
    }
}
impl Drop for Link {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.disconnect();
        }
    }
}

/// P4 input queue: a bounded FIFO tied to a boot and Pane/Popup, never replayed.
#[allow(dead_code)]
pub(crate) struct HeldInput {
    pub boot: String,
    pub target: crate::terminal::InputTarget,
    fifo: std::collections::VecDeque<sheprd_client::protocol::ClientPaneInputEvent>,
}
#[allow(dead_code)]
impl HeldInput {
    pub fn new(boot: String, target: crate::terminal::InputTarget) -> Self {
        Self {
            boot,
            target,
            fifo: Default::default(),
        }
    }
    pub fn push(&mut self, event: sheprd_client::protocol::ClientPaneInputEvent) -> bool {
        if self.fifo.len() >= 64 {
            return false;
        }
        self.fifo.push_back(event);
        true
    }
    pub fn pop(
        &mut self,
        boot: &str,
        target: &crate::terminal::InputTarget,
    ) -> Option<sheprd_client::protocol::ClientPaneInputEvent> {
        if self.boot != boot || &self.target != target {
            self.fifo.clear();
            return None;
        }
        self.fifo.pop_front()
    }
}

impl Link {
    /// Recheck the reader's state at enqueue time, not only a UI polling copy.
    pub(crate) fn send_verified(
        &self,
        destination: &crate::terminal_input::Destination,
        event: sheprd_client::protocol::ClientPaneInputEvent,
    ) -> crate::Result<()> {
        let live = self
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Input state busy; input discarded".into()))?;
        if crate::terminal_input::destination(&live, self.generation).as_ref() != Some(destination)
        {
            return Err(crate::Error::Ssh(
                "Input target changed; input discarded".into(),
            ));
        }
        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected; input discarded".into()))?;
        match &destination.target {
            crate::terminal::InputTarget::Pane(id) => {
                handle.send_input(&destination.boot, id, [event])
            }
            crate::terminal::InputTarget::Popup(id) => {
                handle.send_popup_input(&destination.boot, id, [event])
            }
        }
        .map_err(|e| crate::Error::Ssh(e.to_string()))
    }
}
