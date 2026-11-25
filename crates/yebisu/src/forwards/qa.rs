//! Simulator QA for scripts/ios/forward-check.py (`YEBISU_FORWARD_QA`): the
//! public line of the key a forward signs with, for the harness to authorize
//! on its private sshd. Settings' key reader refuses the device key in QA
//! runs (`key_qa_required`), but a forward always signs with it
//! (`sheprd_client::ssh::forward_connect`), so this run reads its public
//! half once, on a thread of its own, and only on the Simulator.
use std::sync::{Mutex, OnceLock, PoisonError};

static LINE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// The device key's `authorized_keys` line once read; the first call starts
/// the read. `None` without `YEBISU_FORWARD_QA` or while reading.
pub(super) fn device_key_line() -> Option<String> {
    // Only a forward-check.py run asks for the line.
    std::env::var_os("YEBISU_FORWARD_QA")?;
    let slot = LINE.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("yebisu-forward-qa-key".into())
            .spawn(|| {
                let line = sheprd_client::ssh::authorized_key().ok();
                if let Some(slot) = LINE.get() {
                    *slot.lock().unwrap_or_else(PoisonError::into_inner) = line;
                }
            });
        Mutex::new(None)
    });
    slot.lock().unwrap_or_else(PoisonError::into_inner).clone()
}
