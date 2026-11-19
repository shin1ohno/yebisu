//! The key block's own `qa.state` fields (UI test redesign S2, P2).
//!
//! `key_ready` is reported where the key block is drawn. After a Copy, the
//! app reads `UIPasteboard` back itself and publishes how many copies it
//! made (`copied`) and what the pasteboard then held, as a SHA-256 and a
//! length (`pasteboard_sha256`, `pasteboard_bytes`); the same fields go to
//! the observation file as a `p2-pasteboard` receipt. The key line itself is
//! never published or recorded (p2-check.py fails on a `key_line` field): a
//! test compares the digest with the digest of the line its injected
//! provider holds. This replaces reading the pasteboard from outside the app
//! (`simctl pbpaste`, which timed out on hosted runners) as the P2 check.
use crate::app::Smoke;
use serde_json::{Value, json};
use std::cell::Cell;

/// The plain-text type the GPUI clipboard writes (herdr-uikit's pasteboard).
#[cfg(all(target_os = "ios", target_abi = "sim"))]
const TEXT_TYPE: &str = "public.utf8-plain-text";

thread_local! {
    /// Copies made in this process; `qa.state` starts from zero on a relaunch.
    static COPIES: Cell<u64> = const { Cell::new(0) };
}

impl Smoke {
    /// Call after a Copy put the key line on the pasteboard, on GPUI's
    /// foreground thread (`qa_ax::report`). Does nothing unless the layer is on.
    pub(crate) fn qa_ax_key_copied(&self) {
        if !super::enabled() {
            return;
        }
        let copies = COPIES.with(|count| {
            count.set(count.get() + 1);
            count.get()
        });
        let mut receipt = serde_json::Map::new();
        receipt.insert("kind".to_owned(), json!("p2-pasteboard"));
        for (key, value) in copied_fields(copies, read_back().as_deref()) {
            if !super::report(key, || value.clone()) {
                eprintln!("qa_ax: {key} was not published (not the publishing thread)");
            }
            receipt.insert(key.to_owned(), value);
        }
        self.observation.record(Value::Object(receipt));
    }
}

/// The fields after the `copies`-th copy, with the bytes read back from the
/// pasteboard (`None` when it held no plain text: both are then null).
pub(crate) fn copied_fields(copies: u64, read_back: Option<&[u8]>) -> Vec<(&'static str, Value)> {
    vec![
        ("copied", json!(copies)),
        (
            "pasteboard_sha256",
            read_back.map_or(Value::Null, |bytes| sha256_hex(bytes).into()),
        ),
        (
            "pasteboard_bytes",
            read_back.map_or(Value::Null, |bytes| bytes.len().into()),
        ),
    ]
}

/// Lower-case hex SHA-256, as Python's `hashlib.sha256(...).hexdigest()`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The general pasteboard's plain text as bytes. The app wrote it, so iOS
/// does not ask the user before this read (S0 saw the prompt only when the
/// test runner read another app's text).
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn read_back() -> Option<Vec<u8>> {
    use objc2_foundation::NSString;
    use objc2_ui_kit::UIPasteboard;
    let kind = NSString::from_str(TEXT_TYPE);
    UIPasteboard::generalPasteboard()
        .dataForPasteboardType(&kind)
        .map(|data| data.to_vec())
}

/// Only a Simulator build reaches the read (`qa_ax::enabled`).
#[cfg(not(all(target_os = "ios", target_abi = "sim")))]
fn read_back() -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
#[path = "qa_ax_keys_tests.rs"]
mod tests;
