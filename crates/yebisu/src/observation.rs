//! Bounded, off-UI-thread observation for the simulator smoke gate.
use serde_json::Value;
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Queue bound of the app's observation writer. Hosted runs peak at 40
/// records in one second (2026-11-19, runs 44356754798 to 44406830068).
const QUEUE: usize = 4096;
/// How long a record waits for room in a full queue before it is dropped.
/// QA builds only: without a QA variable there is no sender at all.
const FULL_WAIT: Duration = Duration::from_millis(250);
/// Records one app writer puts in its file; it then writes one
/// `observation-limit` record and discards the rest.
const WRITE_LIMIT: usize = 8192;

#[derive(Clone)]
pub(crate) struct Observation {
    sender: Option<SyncSender<Value>>,
    // Per-process observation id; QA tags records so several windows stay separable.
    window: usize,
    /// Records dropped and not yet reported in the file. The next record that
    /// gets in is preceded by one `observation-dropped` record with the count,
    /// so a drop is never silent to the oracle that reads the file.
    dropped: Arc<AtomicU64>,
    full_wait: Duration,
    #[cfg(test)]
    diagnostics: Option<SyncSender<String>>,
}
static NEXT_OBSERVATION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Observation {
    fn with_sender(sender: Option<SyncSender<Value>>, window: usize, full_wait: Duration) -> Self {
        Self {
            sender,
            window,
            dropped: Arc::new(AtomicU64::new(0)),
            full_wait,
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
        Self::with_sender(Some(sender), 0, Duration::ZERO)
    }

    #[cfg(test)]
    pub(crate) fn injected_with_wait(sender: SyncSender<Value>, full_wait: Duration) -> Self {
        Self::with_sender(Some(sender), 0, full_wait)
    }

    pub(crate) fn new() -> Self {
        let p10 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_SETTINGS_QA").is_some();
        let selection =
            cfg!(target_abi = "sim") && std::env::var_os("YEBISU_SELECTION_SETTINGS_QA").is_some();
        let p11 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_P11_QA").is_some();
        let p15 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_P15_QA").is_some();
        let p9 = cfg!(target_abi = "sim") && std::env::var_os("YEBISU_CORE_QA").is_some();
        let p2 = std::env::var_os("YEBISU_P2_OBSERVE").is_some();
        if !p2 && std::env::var_os("YEBISU_SMOKE_OBSERVE").is_none() {
            return Self::with_sender(None, 0, Duration::ZERO);
        }
        let id = NEXT_OBSERVATION.fetch_add(1, Ordering::Relaxed);
        let menu_case = if cfg!(target_abi = "sim") {
            std::env::var("YEBISU_SH1_152_CASE").ok()
        } else {
            None
        };
        let (sender, receiver) = sync_channel::<Value>(QUEUE);
        std::thread::spawn(move || {
            let Some(home) = std::env::var_os("HOME") else {
                return;
            };
            let directory = std::path::PathBuf::from(home).join("Documents");
            let result = (|| -> std::io::Result<()> {
                std::fs::create_dir_all(&directory)?;
                let filename = menu_case.map(|case| format!("sh1-152-{case}-{id}.jsonl"));
                let path = directory.join(if let Some(filename) = filename.as_deref() {
                    filename
                } else if selection {
                    "selection-settings.jsonl"
                } else if p10 {
                    "p10-settings.jsonl"
                } else if p2 {
                    "p2-setup.jsonl"
                } else {
                    "p0-smoke.jsonl"
                });
                let append = p2 || p10 || selection || p11 || p15 || (p9 && filename.is_none());
                write_records(&path, append, receiver)
            })();
            if let Err(error) = result {
                eprintln!("smoke observation writer: {error}");
            }
        });
        let observation = Self::with_sender(Some(sender), id, FULL_WAIT);
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
        let case = std::env::var("YEBISU_SH1_152_CASE").ok();
        let stamp = |value: &mut Value| {
            if let Some(case) = &case {
                value["case"] = case.clone().into();
                value["window"] = self.window.into();
            }
            value["time"] = now.as_secs_f64().into();
        };
        stamp(&mut value);
        let pending = self.dropped.swap(0, Ordering::AcqRel);
        if pending > 0 {
            let mut notice = serde_json::json!({"kind": "observation-dropped", "count": pending});
            stamp(&mut notice);
            if self.send(sender, notice).is_err() {
                self.dropped.fetch_add(pending, Ordering::AcqRel);
            }
        }
        if let Err(reason) = self.send(sender, value) {
            self.dropped.fetch_add(1, Ordering::AcqRel);
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

    /// Sends one record, waiting up to `full_wait` for room in a full queue.
    fn send(&self, sender: &SyncSender<Value>, mut value: Value) -> Result<(), &'static str> {
        let deadline = Instant::now() + self.full_wait;
        loop {
            match sender.try_send(value) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Disconnected(_)) => return Err("disconnected"),
                Err(TrySendError::Full(back)) => {
                    if Instant::now() >= deadline {
                        return Err("full");
                    }
                    value = back;
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
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
    write_records_to(&mut file, receiver)
}

/// The writer loop of `write_records`, over any sink so a test can count the
/// writes it makes.
fn write_records_to(
    file: &mut impl Write,
    receiver: std::sync::mpsc::Receiver<Value>,
) -> std::io::Result<()> {
    write_records_limited(file, receiver, WRITE_LIMIT)
}

/// Writes up to `limit` records. At the limit it writes one
/// `observation-limit` record and keeps receiving without writing, so the
/// senders never see a closed queue and the file says why it stopped.
fn write_records_limited(
    file: &mut impl Write,
    receiver: std::sync::mpsc::Receiver<Value>,
    limit: usize,
) -> std::io::Result<()> {
    for (written, value) in receiver.iter().enumerate() {
        if written == limit {
            write_jsonl_line(
                file,
                &serde_json::json!({"kind": "observation-limit", "written": limit}),
            )?;
            receiver.iter().for_each(drop);
            break;
        }
        write_jsonl_line(file, &value)?;
    }
    Ok(())
}

/// One JSONL record as one complete line in one `write_all`. Every QA JSONL
/// writer that appends to a file another process or thread may also append
/// to uses it. `writeln!(file, "{value}")` on an unbuffered `File` issues one
/// `write` per formatter piece (`{`, `"kind"`, `:`, ...), so a writer that is
/// killed or interleaved between them leaves part of a line. Integration 2
/// run 44236636053 (layout-smoke job 134574290912) read a complete
/// `p2-setup.jsonl` line that failed at column 8, right after `{"kind"`; the
/// likely writer is the app killed by `terminate()` right after `activate()`
/// in testP2KeysAndRestart, followed by the relaunched app's first record
/// (inferred: the line itself was not uploaded). Records are small, so one
/// `write_all` is one `write`.
pub(crate) fn write_jsonl_line(
    file: &mut impl Write,
    value: &impl std::fmt::Display,
) -> std::io::Result<()> {
    let line = format!("{value}\n");
    file.write_all(line.as_bytes())?;
    file.flush()
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
mod drop_tests;
#[cfg(test)]
mod tests;
