//! Test-only observation of existing F19 races. Never a production input fence.
mod peer;
mod transport;
#[cfg(test)]
mod transport_tests;
use crate::terminal_input::Destination;
pub(crate) use peer::{Peer, Transcript};
use serde_json::{Value, json};
use sheprd_backend::ClientPaneInputEvent;
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
pub(crate) use transport::Control;
pub(crate) const WAIT: Duration = Duration::from_secs(6);
pub(crate) const TOTAL: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Key {
    pub case: &'static str,
    pub run: u64,
    pub channel: u32,
    pub destination: Destination,
    pub input_epoch: u64,
}
impl Key {
    pub fn json(&self) -> Value {
        json!({"case_id":self.case,"run_id":self.run,"channel":self.channel,
               "connection_generation":self.destination.generation,
               "destination_identity":format!("{:?}",self.destination),
               "input_epoch":self.input_epoch})
    }
}
#[derive(Clone)]
pub(crate) struct Journal(Arc<Mutex<State>>);
struct State {
    case: &'static str,
    run: u64,
    key: Option<Key>,
    rows: Vec<Value>,
    invalid: bool,
}
impl Journal {
    pub fn new(case: &'static str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(Arc::new(Mutex::new(State {
            case,
            run: NEXT.fetch_add(1, Ordering::SeqCst),
            key: None,
            rows: Vec::new(),
            invalid: false,
        })))
    }
    pub fn bind(&self, destination: Destination, input_epoch: u64) -> Key {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        assert!(state.key.is_none(), "probe key already bound");
        let key = Key {
            case: state.case,
            run: state.run,
            channel: 1,
            destination,
            input_epoch,
        };
        state.key = Some(key.clone());
        key
    }
    pub fn record(&self, kind: &'static str, data: Value) -> io::Result<u64> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.rows.len() >= 512 {
            state.invalid = true;
            return Err(io::Error::other("probe receipt bound exceeded"));
        }
        let sequence = state.rows.len() as u64 + 1;
        let row = json!({"case_id":state.case,"run_id":state.run,
                         "order":sequence,"kind":kind,"data":data});
        state.rows.push(row);
        Ok(sequence)
    }
    pub fn fail(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).invalid = true;
    }
    pub fn rows(&self) -> Vec<Value> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            !state.invalid,
            "invalid/incomplete probe, never zero-leak evidence"
        );
        state.rows.clone()
    }
}

pub(crate) struct AfterCompare {
    pub key: Key,
    pub event: ClientPaneInputEvent,
    pub arrived: mpsc::SyncSender<Key>,
    pub resume: mpsc::Receiver<Key>,
    pub journal: Journal,
}
impl AfterCompare {
    pub fn hold(
        self,
        destination: &Destination,
        event: &ClientPaneInputEvent,
        epoch: u64,
    ) -> crate::Result<()> {
        if destination != &self.key.destination
            || event != &self.event
            || epoch != self.key.input_epoch
        {
            self.journal.fail();
            return Err(crate::Error::CoreState);
        }
        self.journal
            .record("after-compare-arrived", self.key.json())?;
        self.arrived
            .send(self.key.clone())
            .map_err(|_| crate::Error::CoreState)?;
        if self
            .resume
            .recv_timeout(WAIT)
            .map_err(|_| crate::Error::CoreState)?
            != self.key
        {
            self.journal.fail();
            return Err(crate::Error::CoreState);
        }
        self.journal
            .record("after-compare-release", self.key.json())?;
        Ok(())
    }
}
