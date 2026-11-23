//! The press that opens a link: a tap or pointer click on a pane with
//! Command held. A finger's tap reaches GPUI without modifiers, so Command
//! is read from the window too, which hardware keyboard presses keep current
//! (`ModifiersChanged`); a pointer click carries its own.
//!
//! A press it takes reaches nothing else: no focus change, no selection, no
//! keyboard request and no byte for the terminal. A press it does not take
//! (no Command, or no link under it) goes on as before.
use super::{Refusal, Target, pane_hyperlink_at, pane_link_at, plain};
use crate::{actions::KeyStatus, app::Smoke};
use gpui::{Context, MouseDownEvent, Pixels, Point, Window};
use sha2::{Digest, Sha256};

impl Smoke {
    /// Takes a press with Command held over a link in a pane: opens a web
    /// address in the default browser, or says in the status line why the
    /// link opens nothing. Returns whether it took the press.
    pub(crate) fn command_press(
        &mut self,
        event: &MouseDownEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !(event.modifiers.platform || window.modifiers().platform) {
            return false;
        }
        let Some(target) = self.link_target_at(event.position) else {
            return false;
        };
        match target {
            Target::Web(url) => self.open_web_link(&url, cx),
            Target::Refused(refusal) => self.refuse_link(&refusal),
        }
        cx.notify();
        true
    }

    /// What the link under `position` points at, in the shown frame.
    pub(crate) fn link_target_at(&self, position: Point<Pixels>) -> Option<Target> {
        let (_, surface) = self.terminal.coherent_frame()?;
        let (column, row) = self.terminal.geometry.cell_at(&surface, position, None)?;
        // QA: where the press read, against which frame.
        crate::qa_ax::report("url_press", || {
            serde_json::json!({
                "point": [f32::from(position.x), f32::from(position.y)],
                "cell": [column, row],
                "surface": surface.surface_revision,
                "projection": surface.projection_revision,
                "boot": surface.boot_id,
                "text": surface.frame.cells.iter()
                    .skip(usize::from(row) * usize::from(surface.frame.width))
                    .take(usize::from(surface.frame.width))
                    .map(|cell| cell.symbol.as_str())
                    .collect::<String>(),
            })
        });
        match pane_link_at(&surface, column, row) {
            Some(link) => Some(link.target),
            // An explicit hyperlink whose target is not in the frame's table.
            None => pane_hyperlink_at(&surface, column, row)
                .then_some(Target::Refused(Refusal::Malformed)),
        }
    }

    fn link_status(&mut self, text: String) {
        self.actions.key_status = Some(KeyStatus::new(text, &self.terminal, self.board.selection));
    }

    fn open_web_link(&mut self, url: &str, cx: &mut Context<Self>) {
        // GPUI's platform open: on iPadOS UIKit's
        // `openURL:options:completionHandler:` on the main thread (herdr-uikit
        // platform/system.rs), which returns at once; the system opens the
        // default browser. The address is passed as shown, never to a shell.
        cx.open_url(url);
        let host = plain::host(url).unwrap_or_default().to_owned();
        self.link_status(format!("Opening {host} in the browser"));
        let digest: String = Sha256::digest(url.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        // The app's own receipt of the request it made, for the Simulator
        // XCTest; the log names the host only, as the path may hold a token.
        crate::qa_ax::count("url_open");
        crate::qa_ax::report(
            "url_open_last",
            || serde_json::json!({"url": url, "sha256": digest, "host": host}),
        );
        self.observation.record(serde_json::json!({
            "kind": "url-open",
            "host": host,
            "bytes": url.len(),
            "sha256": digest,
        }));
    }

    fn refuse_link(&mut self, refusal: &Refusal) {
        let reason = refusal.reason();
        crate::qa_ax::count("url_refused");
        crate::qa_ax::report("url_refused_last", || serde_json::json!(reason));
        self.observation
            .record(serde_json::json!({"kind": "url-refused", "reason": reason}));
        self.link_status(reason);
    }
}
