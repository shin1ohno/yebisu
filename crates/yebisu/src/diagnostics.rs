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
/// herdr-uikit's input diagnostics (`herdr_uikit::DIAGNOSTIC_TARGET`): which
/// view is first responder and why, keyboard dismissals and GPUI text focus.
/// The same allowlist applies; every one of its codes is in `UIKIT_CODES`.
const UIKIT_TARGET: &str = "herdr_uikit::diagnostics";
/// herdr-uikit's Simulator input receipts (`qa_ax_input.rs`): accepted only
/// by a Simulator QA launch, and never written to the diagnostics log. A
/// device build does not even carry the name.
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
const RECEIPTS: &str = "herdr_uikit::qa_input";
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
    "keep_awake_on",
    "keep_awake_off",
    "keep_awake_refused",
    // Herdr 2 (herdr-core) links: a dial, its outcome, how a link ended, a
    // space removed with its last tab, and each change of what the
    // `herdr-app` document read found, and ended terminals' panes (with the
    // tabs and space that left empty) taken out of the document. Codes
    // only: no space or host name.
    "herdr2_connecting",
    "herdr2_connected",
    "herdr2_dial_failed",
    "herdr2_ended_lost",
    "herdr2_ended_error",
    "herdr2_space_removed",
    "herdr2_doc_readable",
    "herdr2_doc_unreadable",
    "herdr2_doc_unavailable",
    "herdr2_doc_space_ended",
    // A launch with no record (`launch_restore/no_record.rs`), per saved
    // host: `default` dialed, no Herdr 2 space listed, the listing failed;
    // and once per launch, Return is Tap reconnect so nothing is dialed.
    "herdr2_launch_default",
    "herdr2_launch_default_none",
    "herdr2_launch_default_failed",
    "herdr2_launch_default_tap",
    "panic",
];
/// `herdr_uikit::DIAGNOSTIC_CODES`, repeated here because the host build of
/// herdr-uikit is empty; an iOS test compares the two (`diagnostics/tests.rs`).
pub(crate) const UIKIT_CODES: &[&str] = &[
    // keep-sorted start
    "dismissal_application",
    "dismissal_none_did_show",
    "dismissal_none_focus",
    "dismissal_none_press",
    "dismissal_none_show",
    "dismissal_user_resign",
    "dismissal_user_will_hide",
    "keyboard_will_hide",
    "press_dropped_no_text",
    "press_recovered_text",
    "responder_drawing_app_dismissed",
    "responder_drawing_inactive",
    "responder_drawing_unfocused",
    "responder_drawing_user_dismissed",
    "responder_text",
    "text_focus_gained",
    "text_focus_lost",
    // keep-sorted end
];

/// The iOS build fails if `UIKIT_CODES` and the pinned herdr-uikit's list
/// differ, so a code the SDK adds is never silently dropped.
#[cfg(target_os = "ios")]
const _: () = assert!(
    same_codes(UIKIT_CODES, herdr_uikit::DIAGNOSTIC_CODES),
    "UIKIT_CODES must equal herdr_uikit::DIAGNOSTIC_CODES"
);
#[cfg(target_os = "ios")]
const _: () = assert!(same_bytes(
    UIKIT_TARGET.as_bytes(),
    herdr_uikit::DIAGNOSTIC_TARGET.as_bytes()
));

#[cfg(any(target_os = "ios", test))]
const fn same_codes(left: &[&str], right: &[&str]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if !same_bytes(left[index].as_bytes(), right[index].as_bytes()) {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(any(target_os = "ios", test))]
const fn same_bytes(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The static code `value` names, if any list allows it.
fn allowed(value: &str) -> Option<&'static str> {
    CODES
        .iter()
        .chain(UIKIT_CODES)
        .chain(crate::text_gate::CODES)
        .copied()
        .find(|code| *code == value)
}

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
    let capture = Capture {
        codes: sender,
        receipts: receipts_wanted(),
    };
    tracing::subscriber::set_global_default(capture).map_err(|_| crate::Error::LogSubscriber)?;
    // Do not format the panic payload, which can contain host or input data.
    install_panic_hook();
    record("startup");
    Ok(())
}

// The default hook formats panic payloads, including provider/credential errors.
#[cfg(any(target_os = "ios", test))]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|_| {
        record("panic");
        eprintln!("yebisu panic: payload suppressed");
    }));
}

/// Whether this launch takes in herdr-uikit's input receipts: a Simulator
/// QA launch only.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn receipts_wanted() -> bool {
    crate::qa_ax::input::active()
}
/// A device build has no receipt module to call, and takes none.
#[cfg(all(target_os = "ios", not(target_abi = "sim")))]
fn receipts_wanted() -> bool {
    false
}

/// The event targets the subscriber takes: its own codes, the receipts only
/// when `receipts` is set, and herdr-uikit's input diagnostic codes.
fn accepts(target: &str, receipts: bool) -> bool {
    target == TARGET || (receipts && is_receipt(target)) || target == UIKIT_TARGET
}

#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
fn is_receipt(target: &str) -> bool {
    target == RECEIPTS
}
#[cfg(not(any(all(target_os = "ios", target_abi = "sim"), test)))]
fn is_receipt(_: &str) -> bool {
    false
}

struct Capture {
    codes: SyncSender<&'static str>,
    /// Fixed before the subscriber is installed, so the callsite interest
    /// tracing caches from `enabled` is right from the first event.
    receipts: bool,
}
struct Code(Option<&'static str>);
impl Visit for Code {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "code" {
            self.0 = allowed(value);
        }
    }
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}
impl Subscriber for Capture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.is_event() && accepts(metadata.target(), self.receipts)
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
        if is_receipt(event.metadata().target()) {
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            crate::qa_ax::input::capture(event);
            return;
        }
        let mut code = Code(None);
        event.record(&mut code);
        if let Some(code) = code.0 {
            // A full/disconnected queue drops diagnostics; never stalls the UI.
            let _ = self.codes.try_send(code);
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
