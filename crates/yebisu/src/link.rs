//! One reader per connection; reconnect replaces all mailboxes before starting.
use crate::live::Live;
use sheprd_client::{Client, ClientEvent, ClientHandle};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

#[derive(Default)]
pub(crate) struct Link {
    pub generation: u64,
    pub reader: Option<JoinHandle<()>>,
    pub inbox: Arc<Mutex<Live>>,
    pub drained: Arc<AtomicBool>,
    pub handle: Option<ClientHandle>,
}
impl Link {
    pub fn reset(&mut self) -> u64 {
        if let Some(handle) = self.handle.take() {
            handle.disconnect();
        }
        self.reader.take(); // Never join on the UI thread.
        self.generation = self.generation.wrapping_add(1);
        let mut live = Live::default();
        live.status = "Connecting…".into();
        live.dirty = true;
        self.inbox = Arc::new(Mutex::new(live));
        self.drained = Arc::new(AtomicBool::new(false));
        self.generation
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
        if let Ok(mut live) = self.inbox.lock() {
            live.apply(ClientEvent::Disconnected { reason, ssh: None });
        }
        self.drained.store(true, Ordering::Release);
    }
    pub fn take_update(&self) -> Option<Live> {
        let mut live = self.inbox.try_lock().ok()?;
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
