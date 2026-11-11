//! Bounded, off-UI-thread observation for the simulator smoke gate.
use serde_json::Value;
use std::{
    io::Write,
    sync::mpsc::{SyncSender, sync_channel},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct Observation(Option<SyncSender<Value>>);

impl Observation {
    pub(crate) fn new() -> Self {
        let p10 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_SETTINGS_QA").is_some();
        let selection =
            cfg!(target_abi = "sim") && std::env::var_os("YEBISU_SELECTION_SETTINGS_QA").is_some();
        let p2 = std::env::var_os("YEBISU_P2_OBSERVE").is_some();
        if !p2 && std::env::var_os("YEBISU_SMOKE_OBSERVE").is_none() {
            return Self(None);
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
                    .append(p2 || p10 || selection)
                    .truncate(!(p2 || p10 || selection))
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
        let observation = Self(Some(sender));
        observation
            .record(serde_json::json!({"kind": "launch", "phase": if p2 { "P2" } else { "P0" }}));
        observation
    }

    pub(crate) fn record(&self, mut value: Value) {
        let Some(sender) = &self.0 else {
            return;
        };
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return;
        };
        value["time"] = now.as_secs_f64().into();
        if let Err(error) = sender.try_send(value) {
            eprintln!("smoke observation dropped: {error}");
        }
    }
}
