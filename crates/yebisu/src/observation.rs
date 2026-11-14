//! Bounded, off-UI-thread observation for the simulator smoke gate.
use serde_json::Value;
use std::{
    io::Write,
    sync::mpsc::{SyncSender, sync_channel},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub(crate) struct Observation {
    sender: Option<SyncSender<Value>>,
    #[cfg(test)]
    diagnostics: Option<SyncSender<String>>,
}

impl Observation {
    fn with_sender(sender: Option<SyncSender<Value>>) -> Self {
        Self {
            sender,
            #[cfg(test)]
            diagnostics: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn capture_diagnostics(&mut self, sender: SyncSender<String>) {
        self.diagnostics = Some(sender);
    }

    #[cfg(test)]
    pub(crate) fn injected(sender: SyncSender<Value>) -> Self {
        Self::with_sender(Some(sender))
    }

    pub(crate) fn new() -> Self {
        let p10 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_SETTINGS_QA").is_some();
        let selection =
            cfg!(target_abi = "sim") && std::env::var_os("YEBISU_SELECTION_SETTINGS_QA").is_some();
        let p11 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_P11_QA").is_some();
        let p9 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_CORE_QA").is_some();
        let p2 = std::env::var_os("YEBISU_P2_OBSERVE").is_some();
        if !p2 && std::env::var_os("YEBISU_SMOKE_OBSERVE").is_none() {
            return Self::with_sender(None);
        }
        let (sender, receiver) = sync_channel::<Value>(256);
        std::thread::spawn(move || {
            let Some(home) = std::env::var_os("HOME") else {
                return;
            };
            let directory = std::path::PathBuf::from(home).join("Documents");
            let result = (|| -> std::io::Result<()> {
                std::fs::create_dir_all(&directory)?;
                let path = directory.join(if selection {
                    "selection-settings.jsonl"
                } else if p10 {
                    "p10-settings.jsonl"
                } else if p2 {
                    "p2-setup.jsonl"
                } else {
                    "p0-smoke.jsonl"
                });
                write_records(&path, p2 || p10 || selection || p11 || p9, receiver)
            })();
            if let Err(error) = result {
                eprintln!("smoke observation writer: {error}");
            }
        });
        let observation = Self::with_sender(Some(sender));
        observation
            .record(serde_json::json!({"kind": "launch", "phase": if p2 { "P2" } else { "P0" }}));
        observation
    }

    pub(crate) fn record(&self, value: Value) {
        let _ = self.try_record(value);
    }

    // Admission is not durable delivery. QA runners must also observe the sink.
    pub(crate) fn try_record(&self, mut value: Value) -> bool {
        let Some(sender) = &self.sender else {
            return false;
        };
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return false;
        };
        value["time"] = now.as_secs_f64().into();
        if let Err(error) = sender.try_send(value) {
            let reason = match error {
                std::sync::mpsc::TrySendError::Full(_) => "full",
                std::sync::mpsc::TrySendError::Disconnected(_) => "disconnected",
            };
            let message = format!("smoke observation dropped: {reason}");
            #[cfg(test)]
            if let Some(sink) = &self.diagnostics {
                let _ = sink.try_send(message.clone());
            }
            eprintln!("{message}");
            return false;
        }
        true
    }
}

fn write_records(
    path: &std::path::Path,
    append: bool,
    receiver: std::sync::mpsc::Receiver<Value>,
) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(path)?;
    for value in receiver.into_iter().take(8192) {
        writeln!(file, "{value}")?;
        file.flush()?;
    }
    Ok(())
}
#[cfg(test)]
impl Observation {
    pub(crate) fn file_for_test(
        path: std::path::PathBuf,
    ) -> (Self, std::thread::JoinHandle<std::io::Result<()>>) {
        let (sender, receiver) = sync_channel(8);
        let worker = std::thread::spawn(move || write_records(&path, false, receiver));
        (Self::injected(sender), worker)
    }
}

#[cfg(test)]
mod tests;
