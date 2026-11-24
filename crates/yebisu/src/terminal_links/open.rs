//! The press that opens a link: a tap or pointer click on a pane with
//! Command held. A finger's tap reaches GPUI without modifiers, so Command
//! is read from the window too, which hardware keyboard presses keep current
//! (`ModifiersChanged`); a pointer click carries its own.
//!
//! A press it takes reaches nothing else: no focus change, no selection, no
//! keyboard request and no byte for the terminal. A press it does not take
//! (no Command, or no link under it) goes on as before.
use super::{Link, Refusal, Target, pane_hyperlink_at, pane_link_at, plain};
use crate::{actions::KeyStatus, app::Smoke};
use gpui::{Context, MouseDownEvent, Pixels, Point, Window};
use sha2::{Digest, Sha256};
use sheprd_client::protocol::PaneSurfaceFrame;
use std::sync::Arc;

/// The shown frame a press read, the frame cell under it, and the link
/// there, if any.
type Pressed = (Arc<PaneSurfaceFrame>, (u16, u16), Option<Link>);

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
        // The window's Command counts only while the keyboard has spoken
        // since the scene last became active (`Hover::trusts_keyboard`).
        let keyboard = self.terminal.link_hover.trusts_keyboard() && window.modifiers().platform;
        if !(event.modifiers.platform || keyboard) {
            return false;
        }
        let Some((surface, cell, link)) = self.link_at(event.position) else {
            return false;
        };
        match link {
            Some(Link {
                target: Target::Web(_),
                unproven: true,
                ..
            }) => {
                if self.resolve_link(&surface, cell) {
                    self.link_status("Checking where the address ends…".into());
                } else {
                    self.refuse_link(&Refusal::Unproven);
                }
            }
            Some(Link {
                target: Target::Web(url),
                ..
            }) => self.open_web_link(&url, None, cx),
            Some(Link {
                target: Target::Refused(refusal),
                ..
            }) => self.refuse_link(&refusal),
            // An explicit hyperlink whose target is not in the frame's table.
            None if pane_hyperlink_at(&surface, cell.0, cell.1) => {
                self.refuse_link(&Refusal::Malformed)
            }
            None => return false,
        }
        cx.notify();
        true
    }

    /// The shown frame, the cell under `position`, and the link there.
    pub(crate) fn link_at(&self, position: Point<Pixels>) -> Option<Pressed> {
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
        let link = pane_link_at(&surface, column, row);
        Some((surface, (column, row), link))
    }

    fn link_status(&mut self, text: String) {
        self.actions.key_status = Some(KeyStatus::new(text, &self.terminal, self.board.selection));
    }

    /// Opens `url`; `joined` is the whole address when Herdr proved that it
    /// wraps, which the status line then shows, since no row shows it whole.
    pub(super) fn open_web_link(
        &mut self,
        url: &str,
        joined: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        // GPUI's platform open: on iPadOS UIKit's
        // `openURL:options:completionHandler:` on the main thread (herdr-uikit
        // platform/system.rs), which returns at once; the system opens the
        // default browser. The address is passed as shown, never to a shell.
        cx.open_url(url);
        let host = plain::host(url).unwrap_or_default().to_owned();
        self.link_status(match joined {
            Some(whole) => format!("Opening {whole}"),
            None => format!("Opening {host} in the browser"),
        });
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

    pub(super) fn refuse_link(&mut self, refusal: &Refusal) {
        let reason = refusal.reason();
        crate::qa_ax::count("url_refused");
        crate::qa_ax::report("url_refused_last", || serde_json::json!(reason));
        self.observation
            .record(serde_json::json!({"kind": "url-refused", "reason": reason}));
        self.link_status(reason);
    }
}
