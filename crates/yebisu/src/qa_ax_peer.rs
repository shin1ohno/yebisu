//! What the in-process mock peers received, as the `peer` field of
//! `qa.state` (UI test redesign S4: the edit-menu tests).
//!
//! EditMenuQA used to read `p4-peer.jsonl` (Herdr 1's mock peer) and
//! `p9-core-wire.jsonl` (Herdr 2's mock core) while the app was still writing
//! them, to wait until a marker or a pasted clipboard text had arrived. The
//! peers run on their own threads, so they write one shared slot here and the
//! root publisher reads it on the next frame (as `qa_ax::core::layout` does);
//! `qa_ax::request_frame` makes sure that frame comes. The files are still
//! written, and the drivers read them after the app was terminated.
use serde_json::{Value, json};
use std::{collections::VecDeque, sync::Mutex};

/// The pasted texts kept, newest last: a test waits for the one it just
/// caused, and an edit-menu case sends fewer than ten.
pub(crate) const KEEP: usize = 24;
/// A kept text is cut to this many characters (markers and clipboard texts
/// are about 30).
pub(crate) const LONGEST: usize = 64;

#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
const PASTE_START: &[u8] = b"\x1b[200~";
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
const PASTE_END: &[u8] = b"\x1b[201~";

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Received {
    /// Input messages (`ClientShellPaneInput`) the Herdr 1 mock peer received.
    pub(crate) inputs: u64,
    /// The last `KEEP` pasted texts, from either peer.
    pub(crate) pastes: VecDeque<String>,
    /// The last `KEEP` key presses the Herdr 1 mock peer received, as
    /// `<code>/<modifiers>` (#229: a latch and a letter, `Char('c')/2`).
    pub(crate) keys: VecDeque<String>,
}

impl Received {
    pub(crate) fn paste(&mut self, text: &str) {
        let text: String = text.chars().take(LONGEST).collect();
        self.pastes.push_back(text);
        while self.pastes.len() > KEEP {
            self.pastes.pop_front();
        }
    }

    pub(crate) fn key(&mut self, key: String) {
        self.keys.push_back(key);
        while self.keys.len() > KEEP {
            self.keys.pop_front();
        }
    }

    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    pub(crate) fn field(&self) -> Value {
        json!({"inputs": self.inputs, "pastes": self.pastes, "keys": self.keys})
    }
}

static RECEIVED: Mutex<Received> = Mutex::new(Received {
    inputs: 0,
    pastes: VecDeque::new(),
    keys: VecDeque::new(),
});

fn update(change: impl FnOnce(&mut Received)) {
    change(
        &mut RECEIVED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    super::request_frame();
}

/// The Herdr 1 mock peer received one input message carrying `pastes`. Any
/// thread. Does nothing unless the layer is on.
pub(crate) fn input<'a>(pastes: impl IntoIterator<Item = &'a str>) {
    if !super::enabled() {
        return;
    }
    update(|received| {
        received.inputs += 1;
        for text in pastes {
            received.paste(text);
        }
    });
}

/// The Herdr 1 mock peer received these key presses. Any thread. Does
/// nothing unless the layer is on.
pub(crate) fn keys(keys: impl IntoIterator<Item = String>) {
    if !super::enabled() {
        return;
    }
    let keys: Vec<String> = keys.into_iter().collect();
    if keys.is_empty() {
        return;
    }
    update(|received| {
        for key in keys {
            received.key(key);
        }
    });
}

/// The Herdr 2 mock core's wire carried `bytes` to a terminal. Any thread.
/// Does nothing unless the layer is on.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn wire(bytes: &[u8]) {
    if !super::enabled() {
        return;
    }
    let pastes = bracketed(bytes);
    if pastes.is_empty() {
        return;
    }
    update(|received| {
        for text in &pastes {
            received.paste(text);
        }
    });
}

/// The texts of the bracketed pastes (`ESC [200~ ... ESC [201~`) in `bytes`,
/// as the old EditMenuQA matched them in p9-core-wire.jsonl.
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
pub(crate) fn bracketed(bytes: &[u8]) -> Vec<String> {
    let mut texts = Vec::new();
    let mut rest = bytes;
    while let Some(start) = find(rest, PASTE_START) {
        let body = &rest[start + PASTE_START.len()..];
        let Some(end) = find(body, PASTE_END) else {
            break;
        };
        texts.push(String::from_utf8_lossy(&body[..end]).into_owned());
        rest = &body[end + PASTE_END.len()..];
    }
    texts
}

#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The `peer` field: `inputs` and the kept `pastes`.
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
pub(crate) fn field() -> Value {
    RECEIVED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .field()
}

/// Whether the mock peer has received the exact pasted text for this test stage.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn has_paste(text: &str) -> bool {
    RECEIVED
        .try_lock()
        .is_ok_and(|received| received.pastes.iter().any(|paste| paste == text))
}

#[cfg(test)]
#[path = "qa_ax_peer_tests.rs"]
mod tests;
