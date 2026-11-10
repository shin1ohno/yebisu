//! Bounded diagnostics: only static event codes, never application payloads.
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{Metadata, Subscriber, field::Visit, span};

pub(crate) const LOG_LOCATION: &str = include_str!("log-location.txt");
const TARGET: &str = "yebisu::diagnostics";
const QUEUE: usize = 256;
const MAX_BYTES: u64 = 128 * 1024;
const CODES: &[&str] = &[
    "startup",
    "window_opened",
    "window_error",
    "active",
    "foreground",
    "inactive",
    "background",
    "scene_disconnected",
    "connecting",
    "connected",
    "disconnected",
    "connection_error",
    "version_mismatch",
    "command_rejected",
    "setup_error",
    "panic",
];

/// Callers may supply only program-owned static codes. The subscriber checks
/// the allowlist again and ignores all other fields, messages and span data.
pub(crate) fn record(code: &'static str) {
    tracing::info!(target: "yebisu::diagnostics", code);
}

#[cfg(target_os = "ios")]
pub(crate) fn init() -> crate::Result<()> {
    let home = std::env::var_os("HOME").ok_or(crate::Error::LogHome)?;
    let path = PathBuf::from(home).join(LOG_LOCATION.trim());
    let (sender, receiver) = sync_channel(QUEUE);
    std::thread::Builder::new()
        .name("yebisu-log-writer".into())
        .spawn(move || {
            if let Err(error) = write_loop(&path, receiver) {
                eprintln!("yebisu log writer failed: {:?}", error.kind());
            }
        })
        .map_err(crate::Error::Storage)?;
    tracing::subscriber::set_global_default(Capture(sender))
        .map_err(|_| crate::Error::LogSubscriber)?;
    // Do not format the panic payload, which can contain host or input data.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record("panic");
        previous(info);
    }));
    record("startup");
    Ok(())
}

struct Capture(SyncSender<&'static str>);
struct Code(Option<&'static str>);
impl Visit for Code {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "code" {
            self.0 = CODES.iter().copied().find(|code| *code == value);
        }
    }
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}
impl Subscriber for Capture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.is_event() && metadata.target() == TARGET
    }
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }
    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }
        let mut code = Code(None);
        event.record(&mut code);
        if let Some(code) = code.0 {
            // A full/disconnected queue drops diagnostics; never stalls the UI.
            let _ = self.0.try_send(code);
        }
    }
    fn enter(&self, _: &span::Id) {}
    fn exit(&self, _: &span::Id) {}
}

fn write_loop(path: &Path, receiver: Receiver<&'static str>) -> io::Result<()> {
    let mut writer = Writer::open(path)?;
    for code in receiver {
        writer.line(code)?;
    }
    Ok(())
}

struct Writer {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
}
impl Writer {
    fn previous(path: &Path) -> PathBuf {
        path.with_extension("1.jsonl")
    }
    fn open(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Existing oversized files are never retained beyond the size cap.
        for candidate in [path.to_owned(), Self::previous(path)] {
            if candidate.exists() && candidate.metadata()?.len() > MAX_BYTES {
                std::fs::remove_file(candidate)?;
            }
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            path: path.to_owned(),
            file: Some(file),
            bytes,
        })
    }
    fn line(&mut self, code: &'static str) -> io::Result<()> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut line = serde_json::to_vec(&serde_json::json!({
            "event": code, "time": time, "pid": std::process::id()
        }))?;
        line.push(b'\n');
        if self.bytes + line.len() as u64 > MAX_BYTES {
            self.file.take();
            let previous = Self::previous(&self.path);
            if previous.exists() {
                std::fs::remove_file(&previous)?;
            }
            std::fs::rename(&self.path, previous)?;
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
            self.bytes = 0;
        }
        if let Some(file) = &mut self.file {
            file.write_all(&line)?;
            self.bytes += line.len() as u64;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
