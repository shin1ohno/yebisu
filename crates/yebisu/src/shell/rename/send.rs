//! The rename request and its result (issue #143 B-7, B-8).
//!
//! A commit sends one `workspace.rename`, `tab.rename` or `pane.rename`,
//! shaped as herdr-gpui's own clients send it. Its result is matched by
//! request id in `live.rename` (`RenameTicket`), never in the navigation's
//! one slot `live.pending_request`, so navigation and palette commands go on
//! while a rename waits. Until the host answers, the committed name shows
//! at every place the name is drawn, in the muted color (`PendingLabel`);
//! the snapshot itself is never rewritten.
use super::{RenameObject, RenameTarget, normalize::normalize_name};
use crate::{app::Smoke, board::snapshot, shell::style::color};
use gpui::Context;
use std::time::{Duration, Instant};

/// How long a sent name waits for any answer before the host's name shows.
pub(crate) const CONFIRM_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const NOT_CONFIRMED: &str = "Rename not confirmed; showing the host's name";

/// The one rename request in flight on a connection, and Herdr's answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RenameTicket {
    pub(crate) request: String,
    /// `Ok` for a result, `Err(reason)` for an `error` or a rejection.
    pub(crate) outcome: Option<Result<(), String>>,
}

impl RenameTicket {
    /// `Live::apply`'s `Response` for `request_id`: whether it was ours.
    pub(crate) fn answer(
        ticket: &mut Option<Self>,
        request_id: &str,
        response: &serde_json::Value,
    ) -> bool {
        let Some(ticket) = ticket.as_mut().filter(|t| t.request == request_id) else {
            return false;
        };
        ticket.outcome = Some(match response.get("error") {
            Some(error) => Err(error
                .as_str()
                .map_or_else(|| error.to_string(), str::to_owned)),
            None => Ok(()),
        });
        true
    }

    /// `Live::apply`'s `CommandRejected` for `request_id`.
    pub(crate) fn reject(
        ticket: &mut Option<Self>,
        request_id: Option<&str>,
        reason: &str,
    ) -> bool {
        let Some(ticket) = ticket
            .as_mut()
            .filter(|t| Some(t.request.as_str()) == request_id)
        else {
            return false;
        };
        ticket.outcome = Some(Err(reason.to_owned()));
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingState {
    /// Sent; no answer yet.
    Sending,
    /// The host accepted it; shown until a newer snapshot names it.
    Confirmed,
}

/// The committed name shown until the host confirms or refuses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingLabel {
    pub(crate) boot: String,
    pub(crate) object: RenameObject,
    pub(crate) text: String,
    /// The snapshot revision when it was sent; for a Herdr 2 name, the
    /// version of the document read (#144 D3).
    pub(crate) sent_revision: u64,
    /// A Herdr 2 name, written through the `herdr-app` document: no
    /// request answers it; a later read of the document does.
    pub(crate) doc: bool,
    pub(crate) request: String,
    pub(crate) state: PendingState,
    pub(crate) sent_at: Instant,
}

impl PendingLabel {
    pub(crate) fn sending(&self) -> bool {
        self.state == PendingState::Sending
    }
}

impl Smoke {
    /// The name to draw for `object` instead of the host's, with its color:
    /// muted while it is being sent, the normal text once accepted.
    pub(crate) fn rename_label(&self, boot: &str, object: &RenameObject) -> Option<(String, u32)> {
        let pending = self.shell.rename.pending.as_ref()?;
        if pending.boot != boot || &pending.object != object || pending.text.is_empty() {
            return None;
        }
        let ink = if pending.sending() {
            color::MUTED
        } else {
            color::TEXT
        };
        Some((pending.text.clone(), ink))
    }

    /// Sends the committed `text` for `target`; a refusal or an unchanged
    /// name sends nothing.
    pub(crate) fn send_rename(
        &mut self,
        target: &RenameTarget,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let text = normalize_name(text);
        if text.is_empty() && !target.allows_empty() {
            return;
        }
        let sent = match target.kind {
            crate::shell::HerdrKind::Herdr2 => self.send_doc_rename(target, &text),
            crate::shell::HerdrKind::Herdr1 => self.send_rename_inner(target, &text),
        };
        match sent {
            Ok(Some(request)) => {
                let timer = request.clone();
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(CONFIRM_TIMEOUT).await;
                    let _ = this.update(cx, |app, cx| {
                        app.rename_timed_out(&timer);
                        cx.notify();
                    });
                })
                .detach();
            }
            Ok(None) => {}
            Err(error) => self.terminal.live.status = format!("Rename failed: {error}"),
        }
    }

    fn send_rename_inner(
        &mut self,
        target: &RenameTarget,
        text: &str,
    ) -> crate::Result<Option<String>> {
        if self.terminal.reconnect.target != target.host
            || self.terminal.link.generation != target.generation
        {
            return Err(crate::Error::Ssh(super::OTHER_HOST.into()));
        }
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host busy".into()))?;
        let shown =
            snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        if !live.connected
            || live.version_mismatch.get().is_some()
            || shown.boot_id != target.boot
            || !target.object.exists_in(shown)
        {
            return Err(crate::Error::Ssh(target.object.closed_text().into()));
        }
        if !target.object.method().advertised_in(&live.methods) {
            return Err(crate::Error::Ssh(super::NO_METHOD.into()));
        }
        if normalize_name(&target.object.host_name(shown)) == text {
            return Ok(None);
        }
        let revision = shown.revision;
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = handle
            .request(
                &target.boot,
                target.object.method(),
                target.object.params(text),
            )
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        live.rename = Some(RenameTicket {
            request: request.clone(),
            outcome: None,
        });
        drop(live);
        self.shell.rename.pending = Some(PendingLabel {
            boot: target.boot.clone(),
            object: target.object.clone(),
            text: text.to_owned(),
            sent_revision: revision,
            doc: false,
            request: request.clone(),
            state: PendingState::Sending,
            sent_at: Instant::now(),
        });
        self.observation.record(serde_json::json!({
            "kind": "rename",
            "method": target.object.method().as_str(),
        }));
        Ok(Some(request))
    }

    /// A Herdr 2 rename (#144 D3): `Command::RenameSpace` or `RenameTab` to
    /// the `default` core, which writes the document. The name shows,
    /// muted, until a later version of the document is read.
    fn send_doc_rename(
        &mut self,
        target: &RenameTarget,
        text: &str,
    ) -> crate::Result<Option<String>> {
        if let Some(reason) = self.herdr2_rename_unavailable(target) {
            return Err(crate::Error::Ssh(reason.into()));
        }
        let doc = self
            .doc_names()
            .ok_or_else(|| crate::Error::Ssh(super::herdr2::NO_DOCUMENT.into()))?;
        if normalize_name(&doc.name(&target.object)) == text {
            return Ok(None);
        }
        let command = doc
            .command(&target.object, text)
            .ok_or_else(|| crate::Error::Ssh(super::herdr2::NOT_IN_DOCUMENT.into()))?;
        self.try_core_command(command)?;
        // No request id answers a document write: the timer keys on this.
        let request = format!("doc-{}-{}", doc.version, self.shell.rename.next_session);
        self.shell.rename.pending = Some(PendingLabel {
            boot: target.boot.clone(),
            object: target.object.clone(),
            text: text.to_owned(),
            sent_revision: doc.version,
            doc: true,
            request: request.clone(),
            state: PendingState::Sending,
            sent_at: Instant::now(),
        });
        self.observation.record(serde_json::json!({
            "kind": "rename",
            "method": if matches!(target.object, RenameObject::Space(_)) { "doc.rename-space" } else { "doc.rename-tab" },
        }));
        Ok(Some(request))
    }

    /// The four ways a pending name ends (issue B-8): the host's answer and
    /// a newer snapshot, a failure, a disconnect or boot change, and the
    /// 10 s timeout (`rename_timed_out`).
    pub(crate) fn settle_pending_rename(&mut self) {
        let Some(pending) = self.shell.rename.pending.clone() else {
            return;
        };
        let live = &self.terminal.live;
        let shown = snapshot(live);
        if !live.connected || shown.is_none_or(|s| s.boot_id != pending.boot) {
            self.shell.rename.pending = None;
            return;
        }
        if pending.doc {
            // Any later read ends it: it holds the name written, or the
            // document's own after a lost write (whose notice says so).
            if self
                .doc_names()
                .is_none_or(|doc| doc.version > pending.sent_revision)
            {
                self.shell.rename.pending = None;
            }
            return;
        }
        let outcome = live
            .rename
            .as_ref()
            .filter(|t| t.request == pending.request)
            .and_then(|t| t.outcome.clone());
        match outcome {
            Some(Err(reason)) => {
                self.shell.rename.pending = None;
                self.terminal.live.status = format!("Rename failed: {reason}");
            }
            Some(Ok(())) => {
                let newer = shown.is_some_and(|s| s.revision > pending.sent_revision);
                if newer {
                    self.shell.rename.pending = None;
                } else if let Some(p) = self.shell.rename.pending.as_mut() {
                    p.state = PendingState::Confirmed;
                }
            }
            None => {}
        }
    }

    /// No answer to `request` within `CONFIRM_TIMEOUT`.
    pub(crate) fn rename_timed_out(&mut self, request: &str) {
        let waiting = self
            .shell
            .rename
            .pending
            .as_ref()
            .is_some_and(|p| p.request == request && p.sending());
        if waiting {
            self.shell.rename.pending = None;
            self.terminal.live.status = NOT_CONFIRMED.into();
        }
    }
}
