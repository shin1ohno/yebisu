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
        if std::env::var_os("YEBISU_SMOKE_OBSERVE").is_none() {
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
                let mut file = std::fs::File::create(directory.join("p0-smoke.jsonl"))?;
                for value in receiver {
                    writeln!(file, "{value}")?;
                    file.flush()?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("smoke observation writer: {error}");
            }
        });
        Self(Some(sender))
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
