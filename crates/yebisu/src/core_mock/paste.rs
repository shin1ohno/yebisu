use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub(crate) struct Probe {
    pub state: Arc<Mutex<State>>,
    pub closed: Arc<AtomicBool>,
    pub eof: Arc<AtomicBool>,
    pub close: Arc<AtomicBool>,
    pub end: Arc<AtomicBool>,
    pub advertise: bool,
    pub bracketed: bool,
    pub spawn_address: Option<String>,
    #[cfg(test)]
    pub before_read: Arc<super::transport::Latch>,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            state: Arc::default(),
            closed: Arc::default(),
            eof: Arc::default(),
            close: Arc::default(),
            end: Arc::default(),
            advertise: true,
            bracketed: false,
            spawn_address: None,
            #[cfg(test)]
            before_read: Arc::default(),
        }
    }
}
#[derive(Default)]
pub(crate) struct State {
    pub puts: Vec<Value>,
    pub inputs: Vec<(String, Vec<u8>)>,
    pub replies: VecDeque<Option<Value>>,
    pub releases: Vec<(usize, Value)>,
}
impl Probe {
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn reply(&self, request: &Value, bytes: &[u8]) -> (usize, Option<Value>) {
        let mut state = self.lock();
        let index = state.puts.len();
        state.puts.push(json!({"request":request,"bytes":bytes}));
        let reply = state.replies.pop_front().unwrap_or_else(|| {
            Some(json!({"path":format!("/state with space/paste-{index}.png"),"bytes":bytes.len()}))
        });
        (index, reply)
    }
}
pub(super) struct Closed(pub Probe);
impl Drop for Closed {
    fn drop(&mut self) {
        self.0.closed.store(true, Ordering::Release);
    }
}
