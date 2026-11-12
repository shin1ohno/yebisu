//! Bounded, off-UI-thread observation for the simulator smoke gate.
use serde_json::Value;
use std::{
    io::Write,
    sync::mpsc::{SyncSender, sync_channel},
    time::{SystemTime, UNIX_EPOCH},
};

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
        let p15 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_P15_QA").is_some();
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
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(p2 || p10 || selection || p11 || p15)
                    .truncate(!(p2 || p10 || selection || p11 || p15))
                    .open(directory.join(if selection {
                        "selection-settings.jsonl"
                    } else if p10 {
                        "p10-settings.jsonl"
                    } else if p2 {
                        "p2-setup.jsonl"
                    } else {
                        "p0-smoke.jsonl"
                    }))?;
                for value in receiver.into_iter().take(8192) {
                    writeln!(file, "{value}")?;
                    file.flush()?;
                }
                Ok(())
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

    pub(crate) fn record(&self, mut value: Value) {
        let Some(sender) = &self.sender else {
            return;
        };
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return;
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
        }
    }
}

#[cfg(test)]
mod tests;
