//! The mock core's documents (issue #144): `doc.get`, `doc.set` with the
//! version check of herdr-core, `doc` events for every subscribed
//! connection, and the labels `terminal.run` was given.
//!
//! The store belongs to the mock `Host`, so it outlives one connection like
//! herdr-core's documents do. Tests change it from outside (another app's
//! write, with or without an event) and read every `doc.set` it received.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

/// herdr-core refuses larger values with `invalid_params`.
const MAX_VALUE: usize = 1_048_576;
/// The document the official app keeps its layout in.
#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
const APP: &str = "herdr-app";

enum Event {
    Doc {
        name: String,
        version: u64,
    },
    #[cfg(test)]
    Missed,
}

#[derive(Default)]
struct State {
    /// Each name's value and version; a name never written is absent.
    docs: BTreeMap<String, (Value, u64)>,
    /// Every `doc.set` received, accepted or not: its value and version.
    sets: Vec<(Value, u64)>,
    /// `doc.set`s still to find that another app wrote just before them.
    conflicts: usize,
    /// The other app's writes so far: the value of its `"external"` key.
    external: u64,
    events: Vec<Event>,
    labels: BTreeMap<String, Value>,
}

/// One host's documents, shared by every connection to it.
#[derive(Clone, Default)]
pub(crate) struct Store(Arc<Mutex<State>>);

impl Store {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The store a Simulator QA run starts with: `YEBISU_CORE_DOC_QA`
    /// names a fixture of `fixtures/herdr-app/`, and
    /// `YEBISU_CORE_DOC_QA_CONFLICTS=<n>` makes the first `n` writes find
    /// that another app wrote first (`conflict_next`).
    pub(crate) fn qa() -> Self {
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if let Some(value) = std::env::var("YEBISU_CORE_DOC_QA")
            .ok()
            .and_then(|name| qa_fixture(&name))
        {
            let store = Self::default();
            store.write(APP, value, false);
            let conflicts = std::env::var("YEBISU_CORE_DOC_QA_CONFLICTS").ok();
            store.conflict_next(conflicts.and_then(|n| n.parse().ok()).unwrap_or(0));
            return store;
        }
        Self::default()
    }

    /// The answer to a document method, or `None` for any other method.
    /// `Err` is the JSON-RPC error object.
    pub(crate) fn call(&self, method: &str, params: &Value) -> Option<Result<Value, Value>> {
        let name = params["name"].as_str().unwrap_or_default().to_owned();
        match method {
            "doc.get" => {
                let state = self.lock();
                let (value, version) = state.docs.get(&name).cloned().unwrap_or_default();
                Some(Ok(json!({"value": value, "version": version})))
            }
            "doc.set" => {
                let value = params["value"].clone();
                let version = params["version"].as_u64().unwrap_or_default();
                let bytes = serde_json::to_vec(&value).map_or(usize::MAX, |bytes| bytes.len());
                let answer = self.set(&name, value, version, bytes);
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                record(
                    &json!({"method": "doc.set", "name": name, "version": version,
                    "bytes": bytes, "result": answer.as_ref().map_or_else(
                        |error| error["data"]["kind"].clone(), |_| json!("ok"))}),
                );
                Some(answer)
            }
            _ => None,
        }
    }

    fn set(&self, name: &str, value: Value, version: u64, bytes: usize) -> Result<Value, Value> {
        self.external(name);
        let mut state = self.lock();
        state.sets.push((value.clone(), version));
        if bytes > MAX_VALUE {
            return Err(refusal("invalid_params", "the value is larger than 1 MiB"));
        }
        let current = state.docs.get(name).map_or(0, |(_, version)| *version);
        if version != current {
            return Err(refusal("conflict", "the document changed"));
        }
        drop(state);
        Ok(json!({"version": self.write(name, value, true)}))
    }

    /// Stores `value` over the current version; returns the new version.
    fn write(&self, name: &str, value: Value, event: bool) -> u64 {
        let mut state = self.lock();
        let version = state.docs.get(name).map_or(0, |(_, version)| *version) + 1;
        state.docs.insert(name.to_owned(), (value, version));
        if event {
            state.events.push(Event::Doc {
                name: name.to_owned(),
                version,
            });
        }
        version
    }

    /// The next `count` `doc.set`s find that another app wrote first: it
    /// sets a top-level `"external"` key, so each is refused as stale.
    pub(crate) fn conflict_next(&self, count: usize) {
        self.lock().conflicts = count;
    }

    /// The other app's write before a `doc.set`, while `conflict_next` lasts.
    fn external(&self, name: &str) {
        let mut state = self.lock();
        if state.conflicts == 0 {
            return;
        }
        state.conflicts -= 1;
        state.external += 1;
        let external = state.external;
        let (mut value, _) = state.docs.get(name).cloned().unwrap_or_default();
        drop(state);
        if !value.is_object() {
            value = json!({});
        }
        value["external"] = json!(external);
        self.write(name, value, true);
    }

    /// Events so far; a new subscription hears only the later ones.
    pub(crate) fn event_count(&self) -> usize {
        self.lock().events.len()
    }

    /// The event lines after `seen`, which moves past them.
    pub(crate) fn events_after(&self, seen: &mut usize) -> Vec<Vec<u8>> {
        let state = self.lock();
        let lines = state
            .events
            .iter()
            .enumerate()
            .skip(*seen)
            .map(|(index, event)| {
                let seq = index + 1;
                let line = match event {
                    Event::Doc { name, version } => json!({"jsonrpc": "2.0", "method": "event",
                    "params": {"seq": seq, "type": "doc",
                        "data": {"name": name, "version": version}}}),
                    #[cfg(test)]
                    Event::Missed => json!({"jsonrpc": "2.0", "method": "missed",
                    "params": {"seq": seq, "terminals": []}}),
                };
                format!("{line}\n").into_bytes()
            });
        let lines: Vec<_> = lines.collect();
        *seen = state.events.len();
        lines
    }

    /// Keeps the labels a `terminal.run` asked for.
    pub(crate) fn record_labels(&self, address: &str, labels: &Value) {
        if labels.is_object() {
            self.lock()
                .labels
                .insert(address.to_owned(), labels.clone());
        }
    }

    /// A terminal's labels as `terminal.list` reports them.
    pub(crate) fn labels(&self, address: &str) -> Value {
        self.lock()
            .labels
            .get(address)
            .cloned()
            .unwrap_or_else(|| json!({}))
    }
}

/// Test hooks: other apps, lost events, and what the mock received.
#[cfg(test)]
impl Store {
    /// Another app writes `herdr-app`, with its event.
    pub(crate) fn set_external(&self, value: Value) -> u64 {
        self.write(APP, value, true)
    }

    /// A write whose event never arrives (a stream that dropped it).
    pub(crate) fn set_silent(&self, value: Value) -> u64 {
        self.write(APP, value, false)
    }

    /// The subscribed connections hear that events were dropped.
    pub(crate) fn missed(&self) {
        self.lock().events.push(Event::Missed);
    }

    /// `herdr-app` now: its value and version.
    pub(crate) fn doc(&self) -> (Value, u64) {
        self.lock().docs.get(APP).cloned().unwrap_or_default()
    }

    /// Every `doc.set` received, in order.
    pub(crate) fn sets(&self) -> Vec<(Value, u64)> {
        self.lock().sets.clone()
    }
}

fn refusal(kind: &str, message: &str) -> Value {
    json!({"code": -32000, "message": message, "data": {"kind": kind}})
}

/// The fixtures a Simulator QA run can start from.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn qa_fixture(name: &str) -> Option<Value> {
    let text = match name {
        "tree-three-panes" => include_str!("../../fixtures/herdr-app/tree-three-panes.json"),
        "two-spaces" => include_str!("../../fixtures/herdr-app/two-spaces.json"),
        "unknown-fields" => include_str!("../../fixtures/herdr-app/unknown-fields.json"),
        "unreadable-root" => include_str!("../../fixtures/herdr-app/unreadable-root.json"),
        _ => return None,
    };
    serde_json::from_str(text).ok()
}

/// A `doc.set` receipt for p9-check, beside the `terminal.put` ones.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn record(receipt: &Value) {
    if let Some(home) = std::env::var_os("HOME") {
        let path = std::path::PathBuf::from(home).join("Documents/p9-core-wire.jsonl");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = crate::observation::write_jsonl_line(&mut file, receipt);
        }
    }
}
