//! The forward sheet (issue 140, board 42): one saved host's remote port to
//! forward, typed or picked from the ports the host listens on, and that
//! host's forwards. Forward port… opens it from the Host menu, a host row's
//! long press, the Shortcuts list, or a forward row that is not listening.
//!
//! While it is open the terminal receives nothing: the sheet's port field
//! holds the window focus and the terminal's input is detached and fenced,
//! as for the Hosts sheet (`KeyOwner::ForwardSheet`, `footer/keys.rs`).
mod field;
mod view;

pub(crate) use field::{PortEvent, PortField};
pub(crate) use view::state_color;

use crate::{
    app::Smoke,
    forwards::{self, InputError, scan},
};
use gpui::{AppContext as _, Context, Entity, Subscription, Window};

// keep-sorted start
pub(crate) const CHIP_HEIGHT: f32 = 32.;
pub(crate) const CHIP_HIT_HEIGHT: f32 = 44.;
pub(crate) const CLOSE: &str = "Close";
pub(crate) const CLOSE_WIDTH: f32 = 88.;
pub(crate) const FOOTER_HEIGHT: f32 = 68.;
pub(crate) const HEADER_HEIGHT: f32 = 60.;
pub(crate) const LIST_HEADING: &str = "Forwarded ports";
pub(crate) const PORT_LABEL: &str = "Remote port";
/// The sheet's top when the window is Compact or the software keyboard is up.
pub(crate) const RAISED_TOP: f32 = 16.;
pub(crate) const ROW_MIN_HEIGHT: f32 = 44.;
/// The sheet is never wider than the window less this.
pub(crate) const SIDE_MARGIN: f32 = 32.;
pub(crate) const SUBMIT: &str = "Forward";
pub(crate) const TITLE: &str = "Forward port";
pub(crate) const TOP: f32 = 96.;
pub(crate) const WIDTH: f32 = 620.;
// keep-sorted end

/// The sheet's explanation for `host` (its display name).
pub(crate) fn intro(host: &str) -> String {
    format!(
        "Reach a port that a program on {host} listens on from this iPad, over SSH. It stays forwarded until you stop it, and pauses while yebisu is off screen."
    )
}

/// The chips' heading for `host` (its display name).
pub(crate) fn chips_heading(host: &str) -> String {
    format!("Listening on {host}")
}

/// A chip's text: the port with a colon, `:3000`.
pub(crate) fn chip_label(port: u16) -> String {
    format!(":{port}")
}

/// The sheet's width in a window `width` wide: 620, or the window less 32.
pub(crate) fn sheet_width(width: f32) -> f32 {
    WIDTH.min((width - SIDE_MARGIN).max(0.))
}

/// The sheet's top: 96, or 16 in a Compact window or above the keyboard.
pub(crate) fn sheet_top(compact: bool, keyboard_up: bool) -> f32 {
    if compact || keyboard_up {
        RAISED_TOP
    } else {
        TOP
    }
}

/// The open sheet. `host` is set exactly while it is open.
#[derive(Default)]
pub(crate) struct ForwardSheetState {
    /// The saved host's SSH destination.
    pub(crate) host: Option<String>,
    pub(crate) field: Option<Entity<PortField>>,
    pub(crate) error: Option<InputError>,
    pub(crate) selected_chip: Option<u16>,
    /// The host's listening ports; `None` hides the chip row (no answer
    /// yet, or the host has neither `ss` nor `lsof`).
    pub(crate) chips: Option<Vec<u16>>,
    scanner: Option<scan::Scanner>,
    events: Option<Subscription>,
    /// What scans the host instead of SSH; tests set their own.
    pub(crate) scan: Option<scan::Run>,
}

impl std::fmt::Debug for ForwardSheetState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForwardSheetState")
            .field("host", &self.host)
            .field("error", &self.error)
            .field("selected_chip", &self.selected_chip)
            .field("chips", &self.chips)
            .field("scanning", &self.scanner.is_some())
            .finish_non_exhaustive()
    }
}

/// How the app reads a host's listening ports: over SSH on iPadOS.
fn ssh_scan() -> Option<scan::Run> {
    #[cfg(target_os = "ios")]
    {
        Some(scan::ssh())
    }
    #[cfg(not(target_os = "ios"))]
    {
        None
    }
}

impl Smoke {
    pub(crate) fn forward_sheet_open(&self) -> bool {
        self.shell.forward_sheet.host.is_some()
    }

    /// Opens the sheet for the saved host `host`, starts reading its
    /// listening ports, and gives the port field the keys. Another sheet
    /// open, or a host no longer saved, opens nothing.
    pub(crate) fn open_forward_sheet(
        &mut self,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.forward_sheet_open()
            || self.settings_modal
            || self.hosts_sheet_modal()
            || self.new_space_open()
            || !self.setup.hosts.contains(&host)
        {
            return;
        }
        self.shell.shortcuts.open = false;
        self.shell.sidebar.long_press = None;
        let field = cx.new(PortField::new);
        let focus = field.read(cx).focus_handle();
        let events = cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            PortEvent::Submit => {
                let _ = this.submit_forward(window, cx);
            }
            PortEvent::Close => this.close_forward_sheet(window, cx),
        });
        let run = self.shell.forward_sheet.scan.clone().or_else(ssh_scan);
        let sheet = &mut self.shell.forward_sheet;
        sheet.scanner = run.and_then(|run| scan::Scanner::start(host.clone(), run));
        sheet.host = Some(host);
        sheet.field = Some(field);
        sheet.events = Some(events);
        sheet.error = None;
        sheet.selected_chip = None;
        sheet.chips = None;
        self.hand_keys_to_forward_sheet(&focus, window, cx);
        window.request_virtual_keyboard();
        cx.notify();
    }

    /// Closes the sheet; the scan stops and the terminal gets its keys back.
    pub(crate) fn close_forward_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scan = self.shell.forward_sheet.scan.take();
        self.shell.forward_sheet = ForwardSheetState {
            scan,
            ..Default::default()
        };
        self.return_keys_to_terminal(window, cx);
        cx.notify();
    }

    /// Forward: starts forwarding the typed port and closes the sheet, or
    /// shows why not and keeps it open.
    pub(crate) fn submit_forward(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), InputError> {
        let Some(host) = self.shell.forward_sheet.host.clone() else {
            return Ok(());
        };
        let text = self
            .shell
            .forward_sheet
            .field
            .as_ref()
            .map(|field| field.read(cx).text().to_owned())
            .unwrap_or_default();
        let result = forwards::parse_port(&text).and_then(|port| self.forwards.start(&host, port));
        match result {
            Ok(()) => self.close_forward_sheet(window, cx),
            Err(error) => {
                self.shell.forward_sheet.error = Some(error);
                if let Some(field) = &self.shell.forward_sheet.field {
                    field.update(cx, |field, cx| {
                        field.invalid = true;
                        cx.notify();
                    });
                }
                cx.notify();
            }
        }
        result
    }

    /// A chip puts its number in the field; Forward then starts it.
    pub(crate) fn pick_forward_chip(&mut self, port: u16, cx: &mut Context<Self>) {
        let Some(field) = self.shell.forward_sheet.field.clone() else {
            return;
        };
        field.update(cx, |field, cx| {
            field.invalid = false;
            field.set_text(port.to_string(), cx);
        });
        self.shell.forward_sheet.selected_chip = Some(port);
        self.shell.forward_sheet.error = None;
        cx.notify();
    }

    /// `qa.state`'s `forward_sheet`: null while closed, else the host, the
    /// chips, the field's digits and the input error.
    pub(crate) fn forward_sheet_qa(&self, cx: &gpui::App) -> serde_json::Value {
        let sheet = &self.shell.forward_sheet;
        let Some(host) = &sheet.host else {
            return serde_json::Value::Null;
        };
        serde_json::json!({
            "host": host,
            "chips": sheet.chips,
            "field": sheet.field.as_ref().map(|field| field.read(cx).text().to_owned()),
            "error": sheet.error.map(|error| error.to_string()),
        })
    }

    /// Takes the scan's newest answer; true when the chips changed.
    pub(crate) fn poll_forward_sheet(&mut self) -> bool {
        let sheet = &mut self.shell.forward_sheet;
        let Some(chips) = sheet.scanner.as_ref().and_then(scan::Scanner::take) else {
            return false;
        };
        let changed = sheet.chips != chips;
        sheet.chips = chips;
        changed
    }

    /// Open: Safari shows the listening forward's page, with the one-time
    /// hint in the status line. False when the forward does not listen.
    pub(crate) fn open_forward(&mut self, host: &str, remote: u16, cx: &mut Context<Self>) -> bool {
        let Some((url, hint)) = self.forwards.open(host, remote) else {
            return false;
        };
        cx.open_url(&url);
        crate::qa_ax::count("forward_open");
        if let Some(hint) = hint {
            self.notifications.set_status(hint);
        }
        cx.notify();
        true
    }

    /// Stop, or Dismiss once it ended: the forward's row goes away.
    pub(crate) fn stop_forward(&mut self, host: &str, remote: u16, cx: &mut Context<Self>) {
        let ended = self
            .forwards
            .get(host, remote)
            .is_some_and(|forward| matches!(forward.state, forwards::State::Ended(_)));
        let removed = if ended {
            self.forwards.dismiss(host, remote)
        } else {
            self.forwards.stop(host, remote)
        };
        if removed {
            cx.notify();
        }
    }

    /// A forward row's tap: Open when it listens, else the sheet for its
    /// host.
    pub(crate) fn tap_forward_row(
        &mut self,
        host: &str,
        remote: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open_forward(host, remote, cx) {
            self.open_forward_sheet(host.to_owned(), window, cx);
        }
    }
}

#[cfg(test)]
#[path = "forward_sheet_tests.rs"]
mod tests;
